use epoll::{
    Event,
    Events,
    ControlOptions::*,
};
use std::os::fd::AsRawFd;
use std::net::TcpListener;

fn main() {
    let listener = TcpListener::bind("localhost:3000").unwrap();
    listener.set_nonblocking(true).unwrap();

    let epoll = epoll::create(false).unwrap();

    let event = Event::new(Events::EPOLLIN, listener.as_raw_fd() as _);
    epoll::ctl(epoll, EPOLL_CTL_ADD, listener.as_raw_fd(), event).unwrap();
}
