use epoll::{
    Event,
    Events,
    ControlOptions::*,
};
use std::os::fd::AsRawFd;
use std::net::TcpListener;
use std::io;
use std::io::{
    Read,
    Write,
};
use std::collections::HashMap;

enum ConnectionState {
    Read {
        request: [u8; 1024],
        read: usize
    },
    Write {
        response: &'static [u8],
        written: usize,
    },
    Flush,
}

fn main() {
    let listener = TcpListener::bind("localhost:3000").unwrap();
    listener.set_nonblocking(true).unwrap();

    let epoll = epoll::create(false).unwrap();

    let event = Event::new(Events::EPOLLIN, listener.as_raw_fd() as _);
    epoll::ctl(epoll, EPOLL_CTL_ADD, listener.as_raw_fd(), event).unwrap();

    let mut connections = HashMap::new();
    loop {
        let mut events = [Event::new(Events::empty(), 0); 1024];
        let num_events = epoll::wait(epoll, 0, &mut events).unwrap();
        let mut completed = Vec::new();

        'next: for event in &events[..num_events] {
            let fd = event.data as i32;
            // is the listener ready?
            if fd == listener.as_raw_fd() {
                // try accepting a connection
                match listener.accept() {
                    Ok((connection, _)) => {
                        connection.set_nonblocking(true).unwrap();
                        // register the connection with epoll
                        let event = Event::new(Events::EPOLLIN | Events::EPOLLOUT, fd as _);
                        epoll::ctl(epoll, EPOLL_CTL_ADD, fd, event).unwrap();

                        let state = ConnectionState::Read {
                            request: [0u8; 1024],
                            read: 0,
                        };
                        connections.insert(fd, (connection, state));
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => panic!("{e}"),
                }
                continue 'next;
            }
            // otherwise, a connection must be ready
            let (connection, state) = connections.get_mut(&fd).unwrap();

            if let ConnectionState::Read { request, read } = state {
                loop {
                    match connection.read(&mut request[*read..]) {
                        Ok(0) => {
                            println!("client connected unexpectedly");
                            completed.push(fd);
                            continue 'next;
                        }
                        Ok(n) => *read += n,
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue 'next,
                        Err(e) => panic!("{e}"),
                    }
                    if request.get(*read - 4..*read) == Some(b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&request[..*read]);
                println!("{request}");
                let response = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Length: 12\r\n",
                    "Connection: close\r\n",
                    "\r\n",
                    "Hello world!"
                );
                *state = ConnectionState::Write {
                    response: response.as_bytes(),
                    written: 0
                };
            }

            if let ConnectionState::Write { response, written } = state {
                loop {
                    match connection.write(&response[*written..]) {
                        Ok(0) => {
                            println!("client disconnected unexpectedly");
                            completed.push(fd);
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
                    if *written == response.len() {
                        break;
                    }
                }
                *state = ConnectionState::Flush;
            }

            if let ConnectionState::Flush = state {
                match connection.flush() {
                    Ok(_) => {
                        completed.push(fd); // 👈
                    },
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // not ready yet, move on to the next connection
                        continue 'next;
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        }

        for fd in completed {
            let (connection, _state) = connections.remove(&fd).unwrap();
            // unregister from epoll
            drop(connection);
        }
    }
}
