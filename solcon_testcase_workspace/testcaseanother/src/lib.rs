
use std::sync::Mutex;

pub fn fothercrate() {
    let m = Mutex::new(0);
    let mut guard = m.lock().unwrap();
    *guard = 114514;
    println!("{}", *guard);
    println!("fothercrate drop");
    drop(guard);
    println!("fothercrate droped");

    // let m = Mutex::new(0);
    // let mut guard = m.lock().unwrap();
    // *guard = 111;
    // println!("{}", *guard);
} 

