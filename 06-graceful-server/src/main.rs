use signal_hook::consts::signal::SIGINT;
use signal_hook::iterator::Signals;

fn main() {
    println!("Hello, world!");
}

fn ctrl_c() {
    let mut signal = Signals::new(&[SIGINT]).unwrap();
    let _ctrl_c = signal.forever().next().unwrap(); // blocks the thread...
}
