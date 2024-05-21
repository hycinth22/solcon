
use std::sync::{Mutex, LockResult, MutexGuard};
use std::ptr::addr_of;

#[allow(dead_code)]
use super::*;


// mod this_is_our_test_target_mod {
//     pub fn this_is_our_test_target_function<T: std::fmt::Display + Copy>(x: &T) -> T {
//         println!("this_is_our_test_target_function called once {x}");
//         *x
//     }
// }

pub fn this_is_our_test_target_before_handle_function<T: std::fmt::Display>(x: &T) {
    println!("here before test_target called {x}");
}
pub fn this_is_our_test_target_after_handle_function<T: std::fmt::Display>(x: &T, ret: &mut T) {
    println!("here after test_target called {x}");
}
pub fn this_is_our_mutex_lock_before_handle_function<T: ?Sized>(mutex: &Mutex<T>, ) -> () /* LockResult<MutexGuard<'_, T>> */ {
    println!("Mutex locking {:?}, this is before", addr_of!(*mutex));
}
pub fn this_is_our_mutex_lock_after_handle_function<T: ?Sized>(mutex: &Mutex<T>, ret: &mut LockResult<MutexGuard<'_, T>>) -> () /* LockResult<MutexGuard<'_, T>>  */{
    println!("Mutex locking {:?}, this is after", addr_of!(*mutex));
}
pub fn this_is_our_mutexguard_drop_before_handle_function<'a, T: ?Sized + 'a>(guard: &mut MutexGuard<'a, T>) {
    println!("MutexGuard droping {:?}, this is before", addr_of!(*guard));
}
pub fn this_is_our_mutexguard_drop_after_handle_function<'a, T: ?Sized + 'a>(guard: &mut MutexGuard<'a, T>) {
    println!("MutexGuard droping {:?}, this is after", addr_of!(*guard));
}