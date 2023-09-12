use std::collections::HashMap;

// fn spawn<T: Task>(task: T);
trait Task {}

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

    fn poll(&mut self) -> Option<Self::Output>;
    fn poll(&mut self, waker: Waker) -> Option<Self::Output>;
}

impl Scheduler {
    fn spawn<T>(&self, mut future: T) {
        let id = rand();
        // poll the future once to get it started, passing in it's ID
        future.poll(event.id);
        // store the future
        self.tasks.insert(id, future);
    }

    fn run(&self) {
        // loop {
        //     for future in &self.tasks {
        //         future.poll();
        //     }
        // }
        for event in epoll_events {
            // poll the future associated with this event
            let future = self.tasks.get(&event.id).unwrap();
            future.poll(event.id);
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
}

fn main() {
    println!("Hello, world!");
}
