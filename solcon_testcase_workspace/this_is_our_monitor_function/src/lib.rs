
use std::sync::{Mutex, LockResult, MutexGuard};
use std::ptr::addr_of;

#[allow(dead_code)]
#[no_mangle]
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