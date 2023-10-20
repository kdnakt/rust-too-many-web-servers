use std::collections::HashMap;
use std::collections::VecDeque;
use epoll::{
    ControlOptions::EPOLL_CTL_ADD,
    Event,
    Events,
};
use std::sync::{
    Arc,
    Mutex,
};
use std::os::fd::{
    AsRawFd,
    RawFd,
};
use std::cell::RefCell;
use std::net::{
    TcpListener,
    TcpStream,
};
use std::io::{
    ErrorKind,
    Read,
    Write,
};
use std::marker::PhantomData;
// use std::rc::Rc;

// fn spawn<T: Task>(task: T);
// trait Task {}

// simple callback
#[derive(Clone)]
struct Waker(Arc<dyn Fn() + Send + Sync>);

impl Waker {
    fn wake(&self) {
        (self.0)()
    }
}

trait Future {
    type Output;

    // fn poll(&mut self) -> Option<Self::Output>;
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

type SharedTask = Arc<Mutex<dyn Future<Output = ()> + Send>>;

#[derive(Default)]
struct Scheduler {
    // tasks: Mutex<Vec<Box<dyn Future + Send>>>,
    runnable: Mutex<VecDeque<SharedTask>>,
}

static SCHEDULER: Scheduler = Scheduler {
    runnable: Mutex::new(VecDeque::new()),
};

impl Scheduler {
    // fn spawn<T>(&self, mut future: T) {
    //     let id = rand();
    //     // poll the future once to get it started, passing in it's ID
    //     future.poll(event.id);
    //     // store the future
    //     self.tasks.insert(id, future);
    // }
    pub fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) {
        // self.tasks.lock().unwrap().push(Box::new(task));
        self.runnable.lock().unwrap().push_back(Arc::new(Mutex::new(task)));
    }

    // fn run(&self) {
    //     // loop {
    //     //     for future in &self.tasks {
    //     //         future.poll();
    //     //     }
    //     // }
    //     for event in epoll_events {
    //         // poll the future associated with this event
    //         let future = self.tasks.get(&event.id).unwrap();
    //         future.poll(event.id);
    //     }
    // }
    pub fn run(&self) {
        // for task in tasks.lock().unwrap().borrow_mut().iter_mut() {
        //     // TODO:
        // }
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

fn main() {
    println!("Hello, world!");
    // write the tasks that our scheduler is going to run
    // SCHEDULER.spawn(Main::Start);
    SCHEDULER.spawn(listen());
    SCHEDULER.run();
}

// An Async Server
// enum as state mashines
// enum Main {
//     Start,
//     Accept {
//         listener: TcpListener,
//     },
// }

struct WithData<'data, D, F> {
    data: D,
    future: F,
    _data: PhantomData<&'data D>,
}

impl<'data, D, F> WithData<'data, D, F>
where
    F: Future + 'data,
{
    pub fn new(
        data: D,
        construct: impl Fn(&'data D) -> F,
    ) -> WithData<'data, D, F> {
        let future = construct(&data);
        WithData { data, future, _data: PhantomData }
    }
}

impl<'data, D, F> Future for WithData<'data, D, F>
where
    F: Future + 'data,
{
    type Output = F::Output;

    fn poll(&mut self, waker: Waker) -> Option<Self::Output> {
        self.future.poll(waker)
    }
}

struct Handler {
    connection: TcpStream,
    state: HandlerState,
}

enum HandlerState {
    Start,
    Read {
        request: [u8; 1024],
        read: usize,
    },
    Write {
        response: &'static [u8],
        written: usize,
    },
    Flush,
}

/**
impl Future for Main {
    type Output = ();

    fn poll(&mut self, waker: Waker) -> Option<()> {
        if let Main::Start = self {
            let listener = TcpListener::bind("localhost:3000").unwrap();
            listener.set_nonblocking(true).unwrap();
            // register the listener with epoll
            REACTOR.with(|reactor| {
                reactor.add(listener.as_raw_fd(), waker);
            });
            *self = Main::Accept { listener };
        }

        if let Main::Accept { listener } = self {
            match listener.accept() {
                Ok((connection, _)) => {
                    connection.set_nonblocking(true).unwrap();

                    SCHEDULER.spawn(Handler {
                        connection,
                        state: HandlerState::Start,
                    })
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return None;
                }
                Err(e) => panic!("{e}"),
            }
        }

        None
    }
}

impl Future for Handler {
    type Output = ();

    fn poll(&mut self, waker: Waker) -> Option<Self::Output> {
        if let HandlerState::Start = self.state {
            // start by registering our connection for notifications
            REACTOR.with(|reactor| {
                reactor.add(self.connection.as_raw_fd(), waker);
            });

            self.state = HandlerState::Read {
                request: [0u8; 1024],
                read: 0,
            };
        }

        if let HandlerState::Read { request, read } = &mut self.state {
            loop {
                match self.connection.read(&mut request[*read..]) {
                    Ok(0) => {
                        println!("client disconnected unexpectedly");
                        return Some(());
                    }
                    Ok(n) => *read += n,
                    // we can simply return None, knowing that we'll run again once our future is woken.
                    Err(e) if e.kind() == ErrorKind::WouldBlock => return None,
                    Err(e) => panic!("{e}"),
                }

                let read = *read;
                if read >= 4 && &request[read -4..read] == b"\r\n\r\n" {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request[..*read]);
            println!("{}", request);

            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Length: 12\r\n",
                "Connection: close\r\n",
                "\r\n",
                "Hello world!"
            );
            self.state = HandlerState::Write {
                response: response.as_bytes(),
                written: 0,
            };
        }

        if let HandlerState::Write { response, written } = &mut self.state {
            loop {
                match self.connection.write(&response[*written..]) {
                    Ok(0) => println!("client disconnected unexpectedly"),
                    Ok(n) => *written += n,
                    Err(e) if e.kind() == ErrorKind::WouldBlock => continue,
                    Err(e) => panic!("{e}"),
                }
                if *written == response.len() {
                    break;
                }
            }
            self.state = HandlerState::Flush;
        }

        if let HandlerState::Flush = self.state {
            match self.connection.flush() {
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::WouldBlock => return None,
                Err(e) => panic!("{e}"),
            }
        }

        REACTOR.with(|reactor| {
            reactor.remove(self.connection.as_raw_fd());
        });

        Some(())
    }
}
*/

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
    struct FromFn<F>(F);

    impl<F, T> Future for FromFn<F>
    where F: FnMut(Waker) -> Option<T>,
    {
        type Output = T;
        fn poll(&mut self, waker: Waker) -> Option<Self::Output> {
            (self.0)(waker)
        }
    }

    FromFn(f)
}

enum Chain<T1, F, T2> {
    First { future1: T1, transition: Option<F> },
    Second { future2: T2 },
}

fn listen() -> impl Future<Output = ()> {
    poll_fn(|waker| {
        let listener = TcpListener::bind("localhost:3000").unwrap();

        listener.set_nonblocking(true).unwrap();

        REACTOR.with(|reactor| {
            reactor.add(listener.as_raw_fd(), waker);
        });

        Some(listener)
    })
    .chain(|listener| {
        poll_fn(move |_| match listener.accept() {
            Ok((connection, _)) => {
                connection.set_nonblocking(true).unwrap();

                SCHEDULER.spawn(handle(connection));

                None
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => None,
            Err(e) => panic!("{e}"),
       })
    })
}

fn handle(con: TcpStream) -> impl Future<Output = ()> {
    let conn = Arc::new(Mutex::new(con));
    // let conn = Rc::new(con);
    let read_connection_ref = conn.clone();
    // let write_connection_ref = conn.clone();
    // let flush_connection_ref = conn.clone();
    WithData::new(conn, |connection| {
        poll_fn(move |waker| {
            REACTOR.with(|reactor| {
                reactor.add(connection.lock().unwrap().as_raw_fd(), waker);
            });
            Some(())
        })
        .chain(move |_| {
            let mut read = 0;
            let mut request = [0u8; 1024];

            poll_fn(move |_| {
                let connection = &*read_connection_ref;
                loop {
                    // try reading from the stream
                    match connection.lock().unwrap().read(&mut request[read..]) {
                        Ok(0) => {
                            println!("client disconnected unexpectedly");
                            return Some(());
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
                Some(())
            })
        })
        .chain(move |_| {
            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Length: 12\r\n",
                "Connection: close\r\n",
                "\r\n",
                "Hello world!"
            );
            let mut written = 0;

            poll_fn(move |_| {
                // let connection = &*write_connection_ref;
                loop {
                    match connection.clone().lock().unwrap().write(response[written..].as_bytes()) {
                        Ok(0) => {
                            println!("client disconnected unexpectedly");
                            return Some(());
                        }
                        Ok(n) => written += n,
                        Err(e) if e.kind() == ErrorKind::WouldBlock => return None,
                        Err(e) => panic!("{e}"),
                    }
                    if written == response.len() {
                        break;
                    }
                }
                Some(())
            })
        })
        .chain(move |_| {
            poll_fn(move |_| {
                // let connection = &*flush_connection_ref;
                match connection.clone().lock().unwrap().flush() {
                    Ok(_) => {}
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        return None;
                    }
                    Err(e) => panic!("{e}"),
                }

                REACTOR.with(|reactor| {
                    // reactor.remove(flush_connection_ref.lock().unwrap().as_raw_fd());
                    reactor.remove(connection.clone().lock().unwrap().as_raw_fd());
                });
                Some(())
            })
        })
    })
}
