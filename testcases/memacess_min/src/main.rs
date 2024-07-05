use std::sync::Mutex;
use std::thread;

fn main() {
    let mutex = Mutex::new(111);
    let mut guard = mutex.lock().unwrap();
    *guard = 3;
    f(&*guard);
    drop(guard);
    thread::spawn(||{}).join();
}

fn f(x: &i32) {
    let y = *x;
}
