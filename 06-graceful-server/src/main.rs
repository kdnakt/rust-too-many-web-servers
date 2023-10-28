use std::sync::{
    Arc,
    Mutex,
};
use signal_hook::consts::signal::SIGINT;
use signal_hook::iterator::Signals;

fn main() {
    println!("Hello, world!");
}

fn ctrl_c() -> impl Future<Output = ()> {
    spawn_blocking(|| {
        let mut signal = Signals::new(&[SIGINT]).unwrap();
        let _ctrl_c = signal.forever().next().unwrap(); // blocks the thread...
    })
}

fn spawn_blocking(blocking_work: impl FnOnce() + Send + 'static) -> impl Future<Output = ()> {
    let state: Arc<Mutex<(bool, Option<Waker>)>> = Arc::default();
    let state_handle = state.clone();

    // run the blocking work on a separate thread
    std::thread::spawn(move|| {
        // run the work
        blocking_work();

        // mark the task as done
        let (done, waker) = &mut *state_handle.lock().unwrap();
        *done = true;

        // wake the waker
        if let Some(waker) = waker.take() {
            waker.wake();
        }
    });

    poll_fn(move |waker| match &mut *state.lock().unwrap() {
        // work is not completed, store our waker and come back later
        (false, state) => {
            *state = Some(waker);
            None
        }
        // the work is completed
        (true, _) => Some(()),
    })
}

struct Select<L, R> {
    left: L,
    right: R
}

fn select<L, R>(left: L, right: R) -> Select<L, R> {
    Select { left, right }
}

enum Either<L, R> {
    Left(L),
    Right(R)
}

impl<L: Future, R: Future> Future for Select<L, R> {
    type Output = Either<L::Output, R::Output>;

    fn poll(&mut self, waker: Waker) -> Option<Self::Output> {
        if let Some(output) = self.left.poll(waker.clone()) {
            return Some(Either::Left(output));
        }

        if let Some(output) = self.right.poll(waker) {
            return Some(Either::Right(output));
        }

        None
    }
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