#![allow(incomplete_include)]

use std::{hint::black_box, time::Instant};

mod this_is_our_test_target_mod {
    pub fn this_is_our_test_target_function<T: std::fmt::Display>(x: &T) -> i32 {
        println!("this_is_our_test_target_function called once {x}");
        111
    }
}

fn test_target_f() {
    println!("hello test_target_f");
} 

static a:i32 = 2;

fn main() {
    println!("hello case target");
    test_target_f();
    println!("ready to ");
    this_is_our_test_target_mod::this_is_our_test_target_function(&a);
    this_is_our_test_target_mod::this_is_our_test_target_function(&true);
    this_is_our_test_target_mod::this_is_our_test_target_function(&3usize);
    this_is_our_test_target_mod::this_is_our_test_target_function(&4.2);
    test_target_f();
}
