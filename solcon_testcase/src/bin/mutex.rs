
use std::sync::{Arc, Mutex, LockResult, MutexGuard};
use std::ptr::addr_of;


#[allow(dead_code)]
mod this_is_our_monitor_function {
    use super::*;
    //#[rustc_std_internal_symbol]
    pub fn this_is_our_mutex_lock_before_handle_function<T: ?Sized>(mutex: &Mutex<T>, ) -> () /* LockResult<MutexGuard<'_, T>> */ {
        println!("Mutex locking {:?}, this is before", addr_of!(*mutex));
    }
    //#[rustc_std_internal_symbol]
    pub fn this_is_our_mutex_lock_after_handle_function<T: ?Sized>(mutex: &Mutex<T>, ret: &mut LockResult<MutexGuard<'_, T>>) -> () /* LockResult<MutexGuard<'_, T>>  */{
        println!("Mutex locking {:?}, this is after", addr_of!(*mutex));
    }
    pub fn this_is_non_generic_func(mutex: i32) -> () {

    }
    //#[no_mangle]
    // pub fn this_is_our_mutex_lock_before_handle_function(ptr:  *const std::ffi::c_void) -> () {
    //     println!("Mutex locking {:?}, this is before", ptr);
    // }
    //#[no_mangle]
    // pub fn this_is_our_mutex_lock_after_handle_function(ptr:  *const std::ffi::c_void, ret:  *const std::ffi::c_void) -> () {
    //     println!("Mutex locking {:?}, this is after", ptr);
    // }
    // pub fn this_is_our_mutexguard_drop_before_handle_function<'a, T: ?Sized + 'a>(guard: &mut MutexGuard<'a, T>) {
    //     println!("MutexGuard droping {:?}, this is before", addr_of!(*guard));
    // }
    // pub fn this_is_our_mutexguard_drop_after_handle_function<'a, T: ?Sized + 'a>(guard: &mut MutexGuard<'a, T>) {
    //     println!("MutexGuard droping {:?}, this is after", addr_of!(*guard));
    // }
}


fn f<T: std::fmt::Display>(m: Arc<Mutex<T>>) {
    let mut guard = m.lock().unwrap();
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
}


// pub fn used_dir() {
//         this_is_our_monitor_function::this_is_our_mutex_lock_before_handle_function(unsafe{
//         &*(0 as *const Mutex<i32>)
//     });
//     this_is_our_monitor_function::this_is_our_mutex_lock_after_handle_function(unsafe{
//         &*(0 as *const Mutex<i32>)
//     }, unsafe {
//         &mut *(0 as *mut LockResult<MutexGuard<'_, i32>>)
//     });
// }

fn main() {

    // this_is_our_monitor_function::this_is_our_mutex_lock_before_handle_function(unsafe{
    //     &*(0 as *const Mutex<i32>)
    // });
    // this_is_our_monitor_function::this_is_our_mutex_lock_after_handle_function(unsafe{
    //     &*(0 as *const Mutex<i32>)
    // }, unsafe {
    //     &mut *(0 as *mut LockResult<MutexGuard<'_, i32>>)
    // });
    println!("hello case arc mutex re");
    let m1= Arc::new(Mutex::new(0));
    let closure = || {
        f(m1.clone());
    };
    closure();
    f(m1.clone());
    f(m1.clone());
    let m2 = Arc::new(Mutex::new(true));
    let closure = || {
        f(m2.clone());
    };
    closure();
    f(m2.clone());
    f(m1.clone());
    let m3 = Arc::new(Mutex::new(241usize));
    let closure = || {
        f(m3.clone());
    };
    closure();
    f(m3.clone());
    f2();
}
