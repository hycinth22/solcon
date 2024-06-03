
use std::sync::{Arc, Mutex, LockResult, MutexGuard};
use std::ptr::addr_of;

fn f<T: std::fmt::Display>(m: Arc<Mutex<T>>) {
    let mut guard = m.lock().unwrap();
    println!("{}", *guard);
    println!("drop");
    drop(guard);
    println!("droped");
} 

fn f2() {
    let m = Mutex::new(0);
}

pub fn main() {
    println!("hello case arc mutex re");
    let m1= Arc::new(Mutex::new(0));
    let closure = || {
        f(m1.clone());
    };
    closure();
    f(m1.clone());
    f(m1.clone());
    let m2 = Arc::new(Mutex::new(true));
    let closure = || {
        f(m2.clone());
    };
    closure();
    f(m2.clone());
    f(m1.clone());
    let m3 = Arc::new(Mutex::new(241usize));
    let closure = || {
        f(m3.clone());
    };
    closure();
    f(m3.clone());
    f2();
}
