#![allow(unused_variables)]
#![allow(unused_mut)]

use std::sync::Mutex;

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
} 

fn f2() {
    let m = Mutex::new(0);
    let mut guard = m.lock().unwrap();
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
    testcase_anothercrate::fothercrate();
    let a = 123;
    testcase_anothercrate::generic_fun(&a);
    testcase_anothercrate::generic_fun(&a);
    let b = 456;
    testcase_anothercrate::generic_fun(&b);
    testcase_anothercrate::generic_fun(&b);
    testcase_mutex_dyn_type::ff();
}
