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
        println!("droping FF object, {:?}, {:?}", self as *const FF, self.time);
    }
}

impl std::fmt::Display for FF {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ffcontent")
    }
}

fn f1(x: FF) {
    println!("calling f1() object {:?}, {:?}", std::ptr::addr_of!(x), x.time);
    let t = black_box(x);
    this_is_our_test_target_mod::this_is_our_test_target_function(t);
}

fn f2(x: FF) {
    println!("calling f2() object {:?}, {:?}", std::ptr::addr_of!(x), x.time);
    let t = black_box(x);
    this_is_our_test_target_mod::this_is_our_test_target_function(t);
}


fn main() {
    println!("hello case target");
    test_target_f();
    f1( black_box(FF{
        time: Instant::now(),
    }));
    f2( black_box(FF{
        time: Instant::now(),
    }));
    test_target_f();
}
