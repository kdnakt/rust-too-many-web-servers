use std::net::{
    TcpListener,
    TcpStream,
};
use std::io;
use std::io::Read;

fn main() {
    let listener = TcpListener::bind("localhost:3000").unwrap();

    loop {
        let (connection, _) = listener.accept().unwrap();

        if let Err(e) = handle_connection(connection) {
            println!("failed to handle connection: {e}");
        }
    }
}

fn handle_connection(mut connection: TcpStream) -> io::Result<()> {
    let mut read = 0;
    let mut request = [0u8; 1024];

    loop {
        // try reading from the stream
        let num_bytes = connection.read(&mut request[read..])?;
        println!("{:?}", request);

        // keep track of how many bytes we've read
        read += num_bytes;

        // the end of the request
        if request.get(read - 4..read) == Some(b"\r\n\r\n") {
            break;
        }
    }

    Ok(())
}
