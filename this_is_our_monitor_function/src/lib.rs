// note: 
// #[allow(dead_code)]: available on all functions
// #[no_mangle]: only available on non-generic functions, leading to duplicate symbol on generic functions
// #[rustc_std_internal_symbol]: only available on non-generic functions, leading to duplicate symbol on generic functions
#![feature(rustc_attrs)]
#![feature(thread_id_value)]
#![feature(allocator_api)]
#![allow(internal_features)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, LockResult, TryLockResult};
use std::sync::{Barrier, BarrierWaitResult};
use std::sync::{Condvar, WaitTimeoutResult};
use std::ptr::addr_of;
use std::time::Duration;
use once_cell::sync::Lazy;

mod utils;
use utils::ThreadInfo;

thread_local! {
    static THREAD : ThreadInfo = utils::get_current_thread_info();
}

fn print_leading_info(callsite: &str) {
    let timestamp = utils::get_timestamp_nanos();
    THREAD.with(|thread| {
        print!("time:{timestamp} callsite({callsite}) ");
    });
}

macro_rules! my_println_with_callsite {
    ($callsite: expr) => {
        print_leading_info($callsite);
        print!("\n");
    };
    ($callsite:expr, $($arg:tt)*) => {{
        print_leading_info($callsite);
        println!($($arg)*);
    }};
}

macro_rules! my_println {
    () => {
        print!("\n");
    };
    ($($arg:tt)*) => {{
        println!($($arg)*);
    }};
}

pub type GlobalSystemAllocatorType = std::alloc::System;
pub static GLOBAL_SYSTEM_ALLOCATOR : GlobalSystemAllocatorType = GlobalSystemAllocatorType{};

struct MonitorLockedInformation;

static START_TIME: Lazy<chrono::DateTime<chrono::Local>> = Lazy::new(|| chrono::Local::now());
static START_TIME_FORMATED: Lazy<String> = Lazy::new(|| START_TIME.format("%Y-%m-%d_%H-%M-%S%.6f").to_string() );
static MONITOR_LOCK: Lazy<Arc<Mutex<MonitorLockedInformation>, GlobalSystemAllocatorType>> = Lazy::new(|| {
    Arc::new_in(Mutex::new(MonitorLockedInformation{
    }), GLOBAL_SYSTEM_ALLOCATOR)
});

static PROGRAM_EXITED: Lazy<Arc<AtomicBool, GlobalSystemAllocatorType>> = Lazy::new(|| Arc::new_in(AtomicBool::new(false), GLOBAL_SYSTEM_ALLOCATOR));

pub fn this_is_our_entry_fn_before_handle_function() {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    println!("Hello enter program entry fn");
}

pub fn this_is_our_entry_fn_after_handle_function() {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    println!("program entry fn return captured");
}

#[rustc_std_internal_symbol]
pub fn this_is_non_generic_func(callsite: &str, x: &i32) -> () {
    my_println_with_callsite!(callsite, "Hello this_is_non_generic_func {x}.");
}

#[inline(always)]
pub fn this_is_generic_func<T: std::fmt::Display>(callsite: &str, x: &T) {
    my_println_with_callsite!(callsite, "Hello this_is_generic_func {x}");
}

#[inline(always)]
pub fn this_is_our_test_target_before_handle_function<T: std::fmt::Display>(callsite: &str, x: &T) {
    my_println_with_callsite!(callsite, "here before test_target called {}.", x);
}

#[inline(always)]
pub fn this_is_our_test_target_after_handle_function<T: std::fmt::Display>(callsite: &str, x: &T, ret: &mut i32) {
    my_println_with_callsite!(callsite, "here after test_target called {}. ret {}", x, ret);
}

#[inline(always)]
pub fn this_is_our_mutex_lock_before_handle_function<T: ?Sized>(callsite: &str, mutex: &Mutex<T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex_addr =  addr_of!(*mutex);
    my_println_with_callsite!(callsite, "Mutex locking {:?}, this is before.", mutex_addr);
}

#[inline(always)]
pub fn this_is_our_mutex_lock_after_handle_function<T: ?Sized>(callsite: &str, mutex: &Mutex<T>, ret: &mut LockResult<MutexGuard<'_, T>>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex_addr =  addr_of!(*mutex);
    my_println_with_callsite!(callsite, "Mutex locking {:?}, this is after, ret addr {:?}", mutex_addr, addr_of!(*ret));
}

#[inline(always)]
pub fn this_is_our_mutex_try_lock_before_handle_function<T: ?Sized>(callsite: &str, mutex: &Mutex<T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    my_println_with_callsite!(callsite, "Mutex try-locking {:?}, this is before.", addr_of!(*mutex));
}

#[inline(always)]
pub fn this_is_our_mutex_try_lock_after_handle_function<T: ?Sized>(callsite: &str, mutex: &Mutex<T>, ret: &mut TryLockResult<MutexGuard<'_, T>>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let locked = ret.is_ok();
    my_println_with_callsite!(callsite, "Mutex try-locking {:?}, this is after, ret addr {:?}, result {locked}", addr_of!(*mutex), addr_of!(*ret));
}

#[inline(always)]
pub fn this_is_our_mem_read_before_function(addr:usize) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    THREAD.with(|thread| {
        my_println!("mem read {addr} in thread {thread:?}");
    });
}

#[inline(always)]
pub fn this_is_our_mem_write_before_function(addr:usize) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    THREAD.with(|thread| {
        my_println!("mem write {addr} in thread {thread:?}");
    });
}

#[inline(always)]
pub fn this_is_our_mem_atomic_read_before_function(addr:usize) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    THREAD.with(|thread| {
        my_println!("mem atomic-read {addr} in thread {thread:?}");
    });
}

#[inline(always)]
pub fn this_is_our_mem_atomic_write_before_function(addr:usize) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    THREAD.with(|thread| {
        my_println!("mem atomic-write {addr} in thread {thread:?}");
    });
}