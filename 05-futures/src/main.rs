
// fn spawn<T: Task>(task: T);
trait Task {}

trait Future {
    type Output;

    fn run(&mut self) -> Option<Self::Output>;
}

fn main() {
    println!("Hello, world!");
}
