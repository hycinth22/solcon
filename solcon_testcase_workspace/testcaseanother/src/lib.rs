// extern crate this_is_our_monitor_function;

use std::sync::Mutex;


pub fn fothercrate() {
    this_is_our_test_target_mod::this_is_our_test_target_function(&123);
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

