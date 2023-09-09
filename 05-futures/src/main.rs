use std::collections::HashMap;

// fn spawn<T: Task>(task: T);
trait Task {}

trait Future {
    type Output;

    fn poll(&mut self) -> Option<Self::Output>;
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

fn main() {
    println!("Hello, world!");
}
