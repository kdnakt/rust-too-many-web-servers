use epoll::{
    Event,
    Events,
    ControlOptions::*,
};
use std::os::fd::AsRawFd;
use std::net::TcpListener;
use std::io;

fn main() {
    let listener = TcpListener::bind("localhost:3000").unwrap();
    listener.set_nonblocking(true).unwrap();

    let epoll = epoll::create(false).unwrap();

    let event = Event::new(Events::EPOLLIN, listener.as_raw_fd() as _);
    epoll::ctl(epoll, EPOLL_CTL_ADD, listener.as_raw_fd(), event).unwrap();

    loop {
        let mut events = [Event::new(Events::empty(), 0); 1024];
        let timeout = -1; // block forever, until something happens
        let num_events = epoll::wait(epoll, timeout, &mut events).unwrap();

        for event in &events[..num_events] {
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
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => panic!("{e}"),
                }
            }
        }
    }
}
