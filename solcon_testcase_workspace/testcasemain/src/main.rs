// extern crate this_is_our_monitor_function;

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

fn unused() {
    // this_is_our_monitor_function::this_is_our_mutex_lock_before_handle_function(unsafe{
    //     &*(0 as *const Mutex<i32>)
    // });
    // this_is_our_monitor_function::this_is_our_mutex_lock_before_handle_function(unsafe{
    //     &*(0 as *const Mutex<bool>)
    // });
    // this_is_our_monitor_function::this_is_non_generic_func(1i32);
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
    testcase_anothercrate::generic_fun(&123);
    testcase_mutex_dyn_type::ff();
    unused();
}
