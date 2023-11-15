use tokio::net::{
    TcpListener,
    TcpStream,
};
use std::io;

// Spins up the runtime and runs the async code in main
#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    loop {
        let (connection, _) = listener.accept().await.unwrap();

        if let Err(e) = handle_connection(connection).await {
            println!("failed to handle connection: {e}");
        }
    }
}

async fn handle_connection(mut connection: TcpStream) -> io::Result<()> {
    Ok(())
}