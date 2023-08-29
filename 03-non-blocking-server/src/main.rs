use std::net::{
    TcpListener,
    TcpStream,
};
use std::io;
use std::io::{
    Read,
    Write,
};

enum ConnectionState {
    Read {
        request: [u8; 1024],
        read: usize
    },
    Write {
        response: &'static [u8],
        written: usize,
    },
    Flush
}

fn main() {
    let listener = TcpListener::bind("localhost:3000").unwrap();
    // switch to using non-blocking I/O
    listener.set_nonblocking(true).unwrap();

    // keep track of all our active connections
    let mut connections = Vec::new();

    loop {
        match listener.accept() {
            Ok((connection, _)) => {
                // switch to using non-blocking I/O
                connection.set_nonblocking(true).unwrap();
                let state = ConnectionState::Read {
                    request: [0u8; 1024],
                    read: 0,
                };
                connections.push((connection, state));
            },
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // the operation was not performed.
                // just spin until the socket becomes ready.
                continue;
            },
            Err(e) => panic!("{e}"),
        }

        let mut completed = Vec::new();

        'next: for (i, (connection, state)) in connections.iter_mut().enumerate() {
            if let ConnectionState::Read { request, read } = state {
                loop {
                    match connection.read(&mut request[*read..]) {
                        Ok(0) => {
                            println!("client disconnected unexpectedly");
                            completed.push(i);
                            continue 'next;
                        }
                        Ok(n) => {
                            // keep track of how many bytes we've read
                            *read += n
                        },
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // not ready yet, move on to the next connection
                            continue 'next;
                        },
                        Err(e) => panic!("{e}"),
                    }

                    // the end of the request
                    if request.get(*read - 4..*read) == Some(b"\r\n\r\n") {
                        break;
                    }
                }
                // we're done
                let request = String::from_utf8_lossy(&request[..read]);
                println!("{request}");
                // spawn a thread to handle each connection
                std::thread::spawn(|| {
                    if let Err(e) = handle_connection(connection) {
                        println!("failed to handle connection: {e}");
                    }
                });
            }
            if let ConnectionState::Write { response, written } = state {
                // TODO:
            }
            if let ConnectionState::Flush = state {
                // TODO
            }
        }
    }
}

fn handle_connection(connection: &mut TcpStream) -> io::Result<()> {
    let mut read = 0;
    let mut request = [0u8; 1024];

    loop {
        // try reading from the stream
        let num_bytes = connection.read(&mut request[read..])?;

        // the client disconnected
        if num_bytes == 0 {
            println!("client disconnected unexpectedly");
            return Ok(());
        }

        read += num_bytes;

    }


    // Hello World! in HTTP
    let response = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Length: 12\r\n",
        "Connection: close\r\n",
        "\r\n",
        "Hello World!"
    );

    let mut written = 0;

    loop {
        // write the remaining response bytes
        let num_bytes = connection.write(response[written..].as_bytes())?;

        // the client disconnected
        if num_bytes == 0 {
            println!("client disconnected unexpectedly");
            return Ok(());
        }

        written += num_bytes;

        // have we written the whole response yet?
        if written == response.len() {
            break;
        }
    }

    // flush the response to ensure
    connection.flush()
}
