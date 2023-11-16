use tokio;
use tokio::net::{
    TcpListener,
    TcpStream,
};
use tokio::io::AsyncReadExt;
use std::io;

// Spins up the runtime and runs the async code in main
#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    loop {
        let (connection, _) = listener.accept().await.unwrap();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(connection).await {
                println!("failed to handle connection: {e}");
            }
        });
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
    Ok(())
}