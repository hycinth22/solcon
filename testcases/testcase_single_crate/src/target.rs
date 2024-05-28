
mod this_is_our_test_target_mod {
    pub fn this_is_our_test_target_function<T: std::fmt::Display + Copy>(x: &T) -> T {
        println!("this_is_our_test_target_function called once {x}");
        *x
    }
}

#[allow(dead_code)]
mod this_is_our_monitor_function {
    use super::*;
    pub fn this_is_our_test_target_before_handle_function<T: std::fmt::Display>(x: &T) {
        println!("here before test_target called {x}");
    }
    pub fn this_is_our_test_target_after_handle_function<T: std::fmt::Display>(x: &T, ret: &mut T) {
        println!("here after test_target called {x}");
    }
}

fn f() {
    println!("hello f");
} 

pub fn main() {
    println!("hello case target");
    f();
    this_is_our_test_target_mod::this_is_our_test_target_function(&1);
    this_is_our_test_target_mod::this_is_our_test_target_function(&true);
    this_is_our_test_target_mod::this_is_our_test_target_function(&3usize);
    this_is_our_test_target_mod::this_is_our_test_target_function(&4.2);
    f();
}
