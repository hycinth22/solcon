#![allow(incomplete_include)]

mod this_is_our_test_target_mod {
    pub fn this_is_our_test_target_function<T: std::fmt::Display + Copy>(x: &T) -> T {
        println!("this_is_our_test_target_function called once {x}");
        *x
    }
}

fn test_target_f() {
    println!("hello test_target_f");
} 

fn main() {
    println!("hello case target");
    test_target_f();
    this_is_our_test_target_mod::this_is_our_test_target_function(&1);
    this_is_our_test_target_mod::this_is_our_test_target_function(&true);
    this_is_our_test_target_mod::this_is_our_test_target_function(&3usize);
    this_is_our_test_target_mod::this_is_our_test_target_function(&4.2);
    test_target_f();
}
