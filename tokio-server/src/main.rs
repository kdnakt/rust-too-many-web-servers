use tokio;
use tokio::net::{
    TcpListener,
    TcpStream,
};
use tokio::io::{
    AsyncReadExt,
    AsyncWriteExt,
};
use tokio::sync::Notify;
use tokio::select;
use tokio::signal::ctrl_c;
use std::io;
use std::sync::{
    Arc,
    atomic::AtomicUsize,
};
use std::time::Duration;

// Spins up the runtime and runs the async code in main
#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let listener = TcpListener::bind("localhost:3000").await.unwrap();
    let state = Arc::new((AtomicUsize::new(0), Notify::new()));

    loop {
        select! {
            // new incoming connection
            result = listener.accept() => {
                let (connection, _) = result.unwrap();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(connection).await {
                        println!("failed to handle connection: {e}");
                    }
                });
            }
            // ctrl+c signal
            _shutdown = ctrl_c() => {
                let timer = tokio::time::sleep(Duration::from_secs(30));
                let request_counter = state.1.notified();
                select! {
                    _ = timer => {}
                    _ = request_counter = {}
                }
                println!("Gracefully shutting down.");
                return;
            }
        }
    }
}

async fn handle_connection(mut connection: TcpStream) -> io::Result<()> {
    let mut read = 0;
    let mut request = [0u8; 1024];

    loop {
        let num_bytes = connection.read(&mut request[read..]).await?;

        if num_bytes == 0 {
            println!("client disconnected unexpectedly");
            return Ok(());
        }

        read += num_bytes;
        if request.get(read - 4..read) == Some(b"\r\n\r\n") {
            break;
        }
    }

    let request = String::from_utf8_lossy(&request[..read]);
    println!("{request}");

    let response = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Length: 12\r\n",
        "Connection: close\r\n",
        "\r\n",
        "Hello world!"
    );
    let mut written = 0;

    loop {
        let num_bytes = connection.write(response[written..].as_bytes()).await?;
        if num_bytes == 0 {
            println!("client disconnected unexpectedly");
            return Ok(());
        }

        written += num_bytes;
        if written == response.len() {
            break;
        }
    }

    Ok(())
}