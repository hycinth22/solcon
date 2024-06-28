use std::sync::Mutex;
use std::thread;

fn main() {
    println!("create mutex");
    let mutex = Mutex::new(111);
    println!("test mutex lock1");
    let guard = mutex.lock().unwrap();
    drop(guard);
    thread::spawn(move || {
	    println!("test mutex lock2");
	    let guard = mutex.lock().unwrap();
	    drop(guard);
    }).join();
}
