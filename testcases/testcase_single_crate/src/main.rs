#![allow(incomplete_include)]

mod test_mutex;

use std::sync::Mutex;
use test_mutex::main as mutexmain;

fn f() {
    let m = Mutex::new(0);
    let mut guard = m.lock().unwrap();
    *guard = 111;
    println!("{}", *guard);
    println!("drop");
    drop(guard);
    println!("droped");

    let m = Mutex::new(0);
    let mut guard = m.lock().unwrap();
    *guard = 111;
    println!("{}", *guard);
    let m = Mutex::new(false);
    let mut guard = m.lock().unwrap();
    *guard = true;
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
    //testcase_anothercrate::generic_fun(&123);
    mutexmain();
    f();
    f2();
    println!("Hello, world!");
    let e = f111();
    f222();
    let r = || {
        println!("Hello, closure!");
    };
    r();
    let r = move|| {
        println!("Hello, moveclosure! {}", e);
    };
    r();
}
