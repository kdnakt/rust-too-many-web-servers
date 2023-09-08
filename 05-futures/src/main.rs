
// fn spawn<T: Task>(task: T);
trait Task {}

trait Future {
    type Output;

    fn poll(&mut self) -> Option<Self::Output>;
}

impl Scheduler {
    fn run(&self) {
        loop {
            for future in &self.tasks {
                future.poll();
            }
        }
    }
}

fn main() {
    println!("Hello, world!");
}
