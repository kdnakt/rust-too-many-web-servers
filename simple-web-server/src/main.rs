use std::net::{
    TcpListener,
    TcpStream,
};
use std::io;

fn main() {
    let listener = TcpListener::bind("localhost:3000").unwrap();

    loop {
        let (connection, _) = listener.accept().unwrap();

        if let Err(e) = handle_connection(connection) {
            println!("failed to handle connection: {e}");
        }
    }
}

fn handle_connection(connection: TcpStream) -> io::Result<()> {
    Ok(())
}
