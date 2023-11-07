use std::sync::{
    Arc,
    Mutex,
};
use signal_hook::consts::signal::SIGINT;
use signal_hook::iterator::Signals;
use std::io::{
    ErrorKind,
    Read,
    Write,
};
use std::net::{
    TcpStream,
    TcpListener,
};
use std::collections::{
    HashMap,
    VecDeque,
};
use std::cell::RefCell;
use std::os::fd::{
    AsRawFd,
    RawFd,
};
use epoll::{
    Event,
    Events,
    ControlOptions::EPOLL_CTL_ADD,
};
use std::time::Duration;
use std::thread;

fn main() {
    println!("Hello, world!");
    SCHEDULER.spawn(listen());
    SCHEDULER.run();
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

enum Chain<T1, F, T2> {
    First { future1: T1, transition: Option<F> },
    Second { future2: T2 },
}

trait Future {
    type Output;

    fn poll(&mut self, waker: Waker) -> Option<Self::Output>;

    fn chain<F, T>(self, chain: F) -> Chain<Self, F, T>
    where F: FnOnce(Self::Output) -> T,
        T: Future,
        Self: Sized,
    {
        Chain::First {
            future1: self,
            transition: Some(chain),
        }
    }
}

impl<T1, F, T2> Future for Chain<T1, F, T2>
where
    T1: Future,
    F: FnOnce(T1::Output) -> T2,
    T2: Future,
{
    type Output = T2::Output;

    fn poll(&mut self, waker: Waker) -> Option<Self::Output> {
        if let Chain::First { future1, transition } = self {
            // poll the first future
            match future1.poll(waker.clone()) {
                Some(value) => {
                    // first future is done, transition into the second
                    let future2 = (transition.take().unwrap())(value);
                    *self = Chain::Second { future2 };
                }
                None => return None,
            }
        }

        if let Chain::Second { future2 } = self {
            return future2.poll(waker);
        }

        None
    }
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

fn listen() -> impl Future<Output = ()> {
    let tasks = Arc::new(Counter::default());
    let tasks_ref = tasks.clone();

    poll_fn(|waker| {
        let listener = TcpListener::bind("localhost:3000").unwrap();

        listener.set_nonblocking(true).unwrap();

        REACTOR.with(|reactor| {
            reactor.add(listener.as_raw_fd(), waker);
        });

        Some(listener)
    })
    .chain(move |listener| {
        let listen = poll_fn(move |_| match listener.accept() {
            Ok((connection, _)) => {
                connection.set_nonblocking(true).unwrap();

                // increment the counter
                tasks.increment();
                let tasks = tasks.clone();
                let handle_connection = handle(connection).chain(|_| {
                    poll_fn(move |_| {
                        // decrement the counter
                        tasks.decrement();
                        Some(())
                    })
                });

                SCHEDULER.spawn(handle_connection);

                None::<()>
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => None::<()>,
            Err(e) => panic!("{e}"),
        });

        select(listen, ctrl_c())
    })
    .chain(|_ctrl_c| graceful_shutdown(tasks_ref))
}

fn graceful_shutdown(tasks: Arc<Counter>) -> impl Future<Output = ()> {
    poll_fn(|waker| {
        let timer = spawn_blocking(|| thread::sleep(Duration::from_secs(30)));
        let request_counter = tasks.wait_for_zero();
        Some(select(timer, request_counter))
    }).chain(|_| {
        println!("Graceful shutdown complete");
        std::process::exit(0)
    })
}

#[derive(Default)]
struct Counter {
    state: Mutex<(usize, Option<Waker>)>,
}

impl Counter {
    fn increment(&self) {
        let (count, _) = &mut *self.state.lock().unwrap();
        *count += 1;
    }

    fn decrement(&self) {
        let (count, waker) = &mut *self.state.lock().unwrap();
        *count -= 1;

        // we were the last task
        if *count == 0 {
            // wake the waiting task
            if let Some(waker) = waker.take() {
                waker.wake();
            }
        }
    }

    fn wait_for_zero(self: Arc<Self>) -> impl Future<Output = ()> {
        poll_fn(move |waker| {
            match &mut *self.state.lock().unwrap() {
                // work is completed
                (0, _) => Some(()),
                // work is not complete
                (_, state) => {
                    *state = Some(waker);
                    None
                }
            }
        })
    }
}

fn handle(con: TcpStream) -> impl Future<Output = ()> {
    let mut conn = Some(con);
    poll_fn(move |waker| {
        REACTOR.with(|reactor| {
            reactor.add(conn.as_ref().unwrap().as_raw_fd(), waker);
        });
        Some(conn.take())
    })
    .chain(move |mut connection| {
        let mut read = 0;
        let mut request = [0u8; 1024];

        poll_fn(move |_| {
            loop {
                // try reading from the stream
                match connection.as_mut().unwrap().read(&mut request[read..]) {
                    Ok(0) => {
                        println!("client disconnected unexpectedly");
                        return Some(connection.take());
                    }
                    Ok(n) => read += n,
                    Err(e) if e.kind() == ErrorKind::WouldBlock => return None,
                    Err(e) => panic!("{e}"),
                }
                let read = read;
                if read >= 4 && &request[read - 4..read] == b"\r\n\r\n" {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request[..read]);
            println!("{request}");
            Some(connection.take())
        })
    })
    .chain(move |mut connection| {
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Length: 12\r\n",
            "Connection: close\r\n",
            "\r\n",
            "Hello world!"
        );
        let mut written = 0;

        poll_fn(move |_| {
            loop {
                match connection.as_mut().unwrap().write(response[written..].as_bytes()) {
                    Ok(0) => {
                        println!("client disconnected unexpectedly");
                        return Some(connection.take());
                    }
                    Ok(n) => written += n,
                    Err(e) if e.kind() == ErrorKind::WouldBlock => return None,
                    Err(e) => panic!("{e}"),
                }
                if written == response.len() {
                    break;
                }
            }
            Some(connection.take())
        })
    })
    .chain(move |mut connection| {
        poll_fn(move |_| {
            match connection.as_ref().unwrap().flush() {
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    return None;
                }
                Err(e) => panic!("{e}"),
            }

            REACTOR.with(|reactor| {
                reactor.remove(connection.as_ref().unwrap().as_raw_fd());
            });
            Some(())
        })
    })
}

type SharedTask = Arc<Mutex<dyn Future<Output = ()> + Send>>;

#[derive(Default)]
struct Scheduler {
    runnable: Mutex<VecDeque<SharedTask>>,
}

static SCHEDULER: Scheduler = Scheduler {
    runnable: Mutex::new(VecDeque::new()),
};

impl Scheduler {
    pub fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) {
        self.runnable.lock().unwrap().push_back(Arc::new(Mutex::new(task)));
    }

    pub fn run(&self) {
        loop {
            loop {
                // pop a runnable task off the queue
                let Some(task) = self.runnable.lock().unwrap().pop_front() else { break };
                let t2 = task.clone();
                // create a waker that pushes the task back on
                let wake = Arc::new(move || {
                    SCHEDULER.runnable.lock().unwrap().push_back(t2.clone());
                });
                // and poll it
                task.lock().unwrap().poll(Waker(wake));
            }
            // if there are no runnable tasks, block on epoll until something becomes ready
            REACTOR.with(|reactor| reactor.wait());
        }
    }
}

struct Reactor {
    epoll: RawFd,
    tasks: RefCell<HashMap<RawFd, Waker>>,
}

thread_local! {
    static REACTOR: Reactor = Reactor::new();
}

impl Reactor {
    pub fn new() -> Reactor {
        Reactor {
            epoll: epoll::create(false).unwrap(),
            tasks: RefCell::new(HashMap::new()),
        }
    }

    /// Add a file descriptor with read and write interest.
    /// `waker` will be called when the descriptor becomes ready
    pub fn add(&self, fd: RawFd, waker: Waker) {
        let event = epoll::Event::new(Events::EPOLLIN | Events::EPOLLOUT, fd as u64);
        epoll::ctl(self.epoll, EPOLL_CTL_ADD, fd, event).unwrap();
        self.tasks.borrow_mut().insert(fd, waker);
    }

    /// Remove the given descriptor from epoll.
    /// It will no longer receive any notifications.
    pub fn remove(&self, fd: RawFd) {
        self.tasks.borrow_mut().remove(&fd);
    }

    /// Drive tasks forward, blocking forever until an event arrives
    pub fn wait(&self) {
        let mut events = [Event::new(Events::empty(), 0); 1024];
        let timeout = -1; // forever
        let num_events = epoll::wait(self.epoll, timeout, &mut events).unwrap();

        for event in &events[..num_events] {
            let fd = event.data as i32;

            // wake the task
            if let Some(waker) = self.tasks.borrow().get(&fd) {
                waker.wake();
            }
        }
    }
}