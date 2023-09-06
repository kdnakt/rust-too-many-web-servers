use epoll::{
    Event,
    Events,
    ControlOptions::*,
};
use std::os::fd::AsRawFd;
use std::net::TcpListener;
use std::io;
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
        let timeout = -1; // block forever, until something happens
        let num_events = epoll::wait(epoll, timeout, &mut events).unwrap();

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
        }
    }
}
