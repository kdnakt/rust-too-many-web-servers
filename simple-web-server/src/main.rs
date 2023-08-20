use std::net::TcpListener;

fn main() {
    let listener = TcpListener::bind("localhost:3000").unwrap();
}
