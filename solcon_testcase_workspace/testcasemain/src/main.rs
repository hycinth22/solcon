use std::sync::Mutex;
use solcon_anothercrate::*;

fn f() {
    let m = Mutex::new(0);
    let mut guard = m.lock().unwrap();
    *guard = 111;
    println!("{}", *guard);
    println!("drop");
    drop(guard);
    println!("droped");

    // let m = Mutex::new(0);
    // let mut guard = m.lock().unwrap();
    // *guard = 111;
    // println!("{}", *guard);
} 

fn f2() {
    let m = Mutex::new(0);
}
fn f111() -> i32 {
    println!("Hello, f111!");
    f112();
    return 1;
}
fn f112() {
    println!("Hello, f111!");
}
fn f222() {
    println!("Hello, f222!");
}

fn unused() {
    this_is_our_monitor_function::this_is_our_mutex_lock_before_handle_function(unsafe{
        &*(0 as *const Mutex<i32>)
    });
}

fn main() {
    f();
    f2();
    println!("Hello, world!");
    let e = f111();
    f222();
    let r = || {
        println!("Hello, closure!");
    };
    let r = move|| {
        println!("Hello, moveclosure! {}", e);
    };
    solcon_anothercrate::fothercrate();
    unused();
}
