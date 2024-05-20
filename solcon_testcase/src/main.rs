mod this_is_our_monitor_function;

use std::sync::Mutex;

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
    // let m = Mutex::new(false);
    // let mut guard = m.lock().unwrap();
    // *guard = true;
} 

fn f2() {
    //let m = Mutex::new(0);
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
}
