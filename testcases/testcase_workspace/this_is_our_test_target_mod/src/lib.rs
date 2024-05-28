
pub fn this_is_our_test_target_function<T: std::fmt::Display + Copy>(x: &T) -> T {
    println!("this_is_our_test_target_function called once {x}");
    *x
}