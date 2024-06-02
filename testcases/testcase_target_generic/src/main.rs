#![allow(incomplete_include)]

use std::{hint::black_box, time::Instant};

mod this_is_our_test_target_mod {
    pub fn this_is_our_test_target_function<T: std::fmt::Display>(x: T) -> i32 {
        println!("this_is_our_test_target_function called once {x}");
        111
    }
}

fn test_target_f() {
    println!("hello test_target_f");
} 


struct FF {
    time: Instant,
}

impl Drop for FF {
    fn drop(&mut self) {
        println!("droping FF object, {:?}", self as *const FF);
    }
}

impl std::fmt::Display for FF {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ffcontent")
    }
}

fn f(x: FF) {
    let t = black_box(x);
    this_is_our_test_target_mod::this_is_our_test_target_function(t);
}

fn main() {
    println!("hello case target");
    test_target_f();
    f( black_box(FF{
        time: Instant::now(),
    }));
    this_is_our_test_target_mod::this_is_our_test_target_function(&1);
    this_is_our_test_target_mod::this_is_our_test_target_function(&true);
    this_is_our_test_target_mod::this_is_our_test_target_function(&3usize);
    this_is_our_test_target_mod::this_is_our_test_target_function(&4.2);
    test_target_f();
}
