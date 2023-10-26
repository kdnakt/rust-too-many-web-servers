use std::sync::{
    Arc,
};
use signal_hook::consts::signal::SIGINT;
use signal_hook::iterator::Signals;

fn main() {
    println!("Hello, world!");
}

fn ctrl_c() {
    let mut signal = Signals::new(&[SIGINT]).unwrap();
    let _ctrl_c = signal.forever().next().unwrap(); // blocks the thread...
}

fn spawn_blocking(blocking_work: impl FnOnce() + Send + 'static) -> impl Future<Output = ()> {
    poll_fn(|waker| {
        // TODO
        None
    })
}

#[derive(Clone)]
struct Waker(Arc<dyn Fn() + Send + Sync>);

impl Waker {
    fn wake(&self) {
        (self.0)()
    }
}

trait Future {
    type Output;

    fn poll(&mut self, waker: Waker) -> Option<Self::Output>;
}

fn poll_fn<F, T>(f: F) -> impl Future<Output = T>
where F: FnMut(Waker) -> Option<T>,
{
    struct PollFn<F>(F);

    impl<F, T> Future for PollFn<F>
    where F: FnMut(Waker) -> Option<T>,
    {
        type Output = T;
        fn poll(&mut self, waker: Waker) -> Option<Self::Output> {
            (self.0)(waker)
        }
    }

    PollFn(f)
}