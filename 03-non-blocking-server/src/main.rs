use std::net::{
    TcpListener,
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
                let request = String::from_utf8_lossy(&request[..*read]);
                println!("{request}");

                // move into the write state
                // Hello World! in HTTP
                let response = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Length: 12\r\n",
                    "Connection: close\r\n",
                    "\r\n",
                    "Hello World!"
                );
                *state = ConnectionState::Write {
                    response: response.as_bytes(),
                    written: 0,
                };
            }
            if let ConnectionState::Write { response, written } = state {
                loop {
                    match connection.write(&response[*written..]) {
                        Ok(0) => {
                            println!("client disconnected unexpectedly");
                            completed.push(i);
                            continue 'next;
                        }
                        Ok(n) => {
                            *written += n;
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // not ready yet, move on to the next connection
                            continue 'next;
                        }
                        Err(e) => panic!("{e}"),
                    }
                    // did we write the whole response yet?
                    if *written == response.len() {
                        break;
                    }
                }
                // successfully wrote the response, try flushing next
                *state = ConnectionState::Flush;
            }
            if let ConnectionState::Flush = state {
                match connection.flush() {
                    Ok(_) => {
                        completed.push(i);
                    },
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        continue 'next;
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        }

        // iterate in reverse order to preserve the indices
        for i in completed.into_iter().rev() {
            connections.remove(i);
        }
    }
}
