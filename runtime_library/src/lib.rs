// note: 
// #[allow(dead_code)]: available on all functions
// #[no_mangle]: only available on non-generic functions, leading to duplicate symbol on generic functions
// #[rustc_std_internal_symbol]: only available on non-generic functions, leading to duplicate symbol on generic functions
#![feature(rustc_attrs)]
#![feature(thread_id_value)]
#![feature(allocator_api)]
#![feature(btreemap_alloc)]
#![allow(internal_features)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_mut)]
use std::alloc::System;
use std::cell::RefCell;
use std::hash::RandomState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, LockResult, TryLockResult};
use std::sync::{Barrier, BarrierWaitResult};
use std::sync::{Condvar, WaitTimeoutResult};
use std::ptr::addr_of;
use std::mem::transmute_copy;
use std::panic::Location;
use std::thread;
use std::time::Duration;
use hashbrown::hash_map::DefaultHashBuilder;
use hashbrown::HashMap;
use memalloc::{GlobalSystemAllocatorType, GLOBAL_SYSTEM_ALLOCATOR};
use once_cell::sync::Lazy;

mod memalloc;
mod locktree;
mod exposed;
mod utils;
mod recorder;
mod reporter;
use utils::{ThreadId, ThreadInfo};

use locktree::*;
use exposed::*;
use recorder::{MemAccessRecord, MemAllocRecord, SyncRecord};

thread_local! {
    static THREAD : ThreadInfo = utils::get_current_thread_info();
}

fn print_leading_info(callsite: &str) {
    let timestamp = utils::get_timestamp_nanos();
    THREAD.with(|thread| {
        print!("time:{timestamp} thread#{tid}({tname}) callsite({callsite}) ", tid=thread.id, tname=thread.name);
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

struct MonitorLockedInformation {
    lock_trace: recorder::RecordWriter<SyncRecord>,
    sync_trace: recorder::RecordWriter<SyncRecord>,
    memaccess_trace: recorder::RecordWriter<MemAccessRecord>,
    memalloc_trace: Arc<Mutex< recorder::RecordWriter<MemAllocRecord> >, GlobalSystemAllocatorType>,
    locktrees: HashMap<ThreadId, Box<ThreadLockTree, GlobalSystemAllocatorType>, DefaultHashBuilder, allocator_api2::alloc::System>,
}

impl MonitorLockedInformation {
    fn run_locktree_analysis(&mut self) {
        my_println!("running locktree conlict lock analysis");
        let keys: Vec<&ThreadId> = self.locktrees.keys().collect();
        my_println!("locktree cnt {}", keys.len());
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                let (t1_id, t2_id) = (keys[i], keys[j]);
                let (t1_tree, t2_tree) = (self.locktrees.get(t1_id).unwrap(), self.locktrees.get(t2_id).unwrap());
                analyze_potential_conflict_locks(t1_tree, t2_tree);
            }
        }
    }
    fn get_locktree_current_thread(&mut self) -> &mut Box<ThreadLockTree, GlobalSystemAllocatorType> {
        let thread = utils::get_current_thread_info();
        let tid = thread.id;
        self.insert_locktree_for_thread_if_not_exist(thread);
        self.locktrees.get_mut(&tid).unwrap()
    }
    fn insert_locktree_for_thread_if_not_exist(&mut self, thread: ThreadInfo) {
        let tid = thread.id;
        let mut tree = Box::new_in(locktree::ThreadLockTree::new(thread), GLOBAL_SYSTEM_ALLOCATOR);
        if !self.locktrees.contains_key(&tid) {
            self.locktrees.insert(tid, tree);
        }
    }
}

static START_TIME: Lazy<chrono::DateTime<chrono::Local>> = Lazy::new(|| chrono::Local::now());
static START_TIME_FORMATED: Lazy<String> = Lazy::new(|| START_TIME.format("%Y-%m-%d_%H-%M-%S%.6f").to_string() );
static MONITOR_LOCK: Lazy<Arc<Mutex<MonitorLockedInformation>, GlobalSystemAllocatorType>> = Lazy::new(|| {
    Arc::new_in(Mutex::new(MonitorLockedInformation{
        lock_trace: recorder::create_lock_logger(START_TIME_FORMATED.as_str()),
        sync_trace: recorder::create_sync_logger(START_TIME_FORMATED.as_str()),
        memaccess_trace: recorder::create_memaccess_logger(START_TIME_FORMATED.as_str()),
        memalloc_trace: Arc::new_in(Mutex::new(recorder::create_memalloc_logger(START_TIME_FORMATED.as_str())), GLOBAL_SYSTEM_ALLOCATOR),
        locktrees: HashMap::new_in(allocator_api2::alloc::System),
    }), GLOBAL_SYSTEM_ALLOCATOR)
});

static PROGRAM_EXITED: Lazy<Arc<AtomicBool, GlobalSystemAllocatorType>> = Lazy::new(|| Arc::new_in(AtomicBool::new(false), GLOBAL_SYSTEM_ALLOCATOR));

fn program_starting(mut lock: MutexGuard<MonitorLockedInformation>) {
    println!("program_starting");
    println!("entrypoint fullstack {}", utils::get_full_stacktrace_str(2));
    memalloc::register_memalloc_tracker(Arc::clone(&MONITOR_LOCK), Arc::clone(&lock.memalloc_trace));
    ctrlc::set_handler(move || {
        let mut lock = MONITOR_LOCK.lock().unwrap();
        PROGRAM_EXITED.store(false, Ordering::Release);
        println!("ctrlc captured");
        program_exiting(lock);
        println!("program exited by Ctrl-C handler");
        std::process::exit(1); // Note that because this function never returns, and that it terminates the process, no destructors on the current stack or any other thread’s stack will be run. Rust IO buffers (eg, from BufWriter) will not be flushed. Likewise, C stdio buffers will (on most platforms) not be flushed.
    }).expect("Error setting Ctrl-C handler");
    env_logger::try_init().unwrap();
}

fn program_exiting(mut lock: MutexGuard<MonitorLockedInformation>) {
    println!("program_exiting");
    lock.lock_trace.flush_and_close().unwrap();
    lock.sync_trace.flush_and_close().unwrap();
    lock.memaccess_trace.flush_and_close().unwrap();
    lock.memalloc_trace.lock().unwrap().flush_and_close().unwrap();
    lock.run_locktree_analysis();
    println!("monitors prepared to exit");
    std::mem::forget(lock); // dont unlock MONITOR_LOCK to prevent all any call to our monitors after this point
}

pub fn this_is_our_entry_fn_before_handle_function() {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    println!("Hello enter program entry fn");
    program_starting(lock);
}

pub fn this_is_our_entry_fn_after_handle_function() {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    println!("program entry fn return captured");
    program_exiting(lock);
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

pub fn this_is_our_mem_read_before_handle_function<T: ?Sized>(callsite: &str, var: &T) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
}

pub fn this_is_our_mem_read_after_handle_function<T: ?Sized>(callsite: &str, var: &T) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
}

pub fn this_is_our_mem_write_before_handle_function<T: ?Sized>(callsite: &str, var: &T) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
}

pub fn this_is_our_mem_write_after_handle_function<T: ?Sized>(callsite: &str, var: &T) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
}


pub fn this_is_our_mutex_lock_before_handle_function<T: ?Sized>(callsite: &str, mutex: &Mutex<T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Mutex locking {:?}, this is before.", mutex_addr);
    THREAD.with(|thread| {
        lock.lock_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: mutex_addr_u64,
            operation_type: 1,
        }).unwrap();
    });
    lock.get_locktree_current_thread().record_lock((mutex_addr_u64, LockType::MutexLock));
}

pub fn this_is_our_mutex_lock_after_handle_function<T: ?Sized>(callsite: &str, mutex: &Mutex<T>, ret: &mut LockResult<MutexGuard<'_, T>>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Mutex locking {:?}, this is after, ret addr {:?}", mutex_addr, addr_of!(*ret));
}

pub fn this_is_our_mutex_try_lock_before_handle_function<T: ?Sized>(callsite: &str, mutex: &Mutex<T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex_addr = addr_of!(*mutex)as *const ()  as u64;
    my_println_with_callsite!(callsite, "Mutex try-locking {:?}, this is before.", addr_of!(*mutex));
}

pub fn this_is_our_mutex_try_lock_after_handle_function<T: ?Sized>(callsite: &str, mutex: &Mutex<T>, ret: &mut TryLockResult<MutexGuard<'_, T>>) {
    let mut lock: MutexGuard<MonitorLockedInformation> = MONITOR_LOCK.lock().unwrap();
    let locked = ret.is_ok();
    if locked {
        let mutex_addr = addr_of!(*mutex);
        let mutex_addr_u64 = mutex_addr as *const () as u64;
        THREAD.with(|thread| {
            lock.lock_trace.write_record(&SyncRecord{
                timestamp: utils::get_timestamp_nanos(),
                thread_id: thread.id,
                memory_address: mutex_addr_u64,
                operation_type: 1,
            }).unwrap();
        });
        lock.get_locktree_current_thread().record_lock((mutex_addr_u64, LockType::MutexLock));
    }
    my_println_with_callsite!(callsite, "Mutex try-locking {:?}, this is after, ret addr {:?}, result {locked}", addr_of!(*mutex), addr_of!(*ret));
}

pub fn this_is_our_mutexguard_drop_before_handle_function<'a, T: ?Sized + 'a>(callsite: &str, guard: &mut MutexGuard<'a, T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "MutexGuard droping {:?}, inner Mutex {:?}, this is before", addr_of!(*guard), addr_of!(*mutex));
    THREAD.with(|thread| {
        lock.lock_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: mutex_addr_u64,
            operation_type: 2,
        }).unwrap();
    });
    lock.get_locktree_current_thread().record_unlock((mutex_addr_u64, LockType::MutexLock));
}

pub fn this_is_our_mutexguard_drop_after_handle_function<'a, T: ?Sized + 'a>(callsite: &str, guard: &mut MutexGuard<'a, T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    my_println_with_callsite!(callsite, "MutexGuard droping {:?}, inner Mutex {:?}, this is after", addr_of!(*guard), addr_of!(*mutex));
}


pub fn this_is_our_rwlock_read_before_handle_function<T: ?Sized>(callsite: &str, rwlock: &RwLock<T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let rwlock_addr = addr_of!(*rwlock);
    let rwlock_addr_u64 = rwlock_addr as *const () as u64;
    my_println_with_callsite!(callsite, "RwLock read-locking {:?}, this is before.", rwlock_addr);
    THREAD.with(|thread| {
        lock.lock_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: rwlock_addr_u64,
            operation_type: 3,
        }).unwrap();
    });
    lock.get_locktree_current_thread().record_lock((rwlock_addr_u64, LockType::ReadLock));
}

pub fn this_is_our_rwlock_read_after_handle_function<T: ?Sized>(callsite: &str, rwlock: &RwLock<T>, ret: &mut LockResult<RwLockReadGuard<'_, T>>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    my_println_with_callsite!(callsite, "RwLock read-locking {:?}, this is after, ret addr {:?}", addr_of!(*rwlock), addr_of!(*ret));
}

pub fn this_is_our_rwlock_try_read_before_handle_function<T: ?Sized>(callsite: &str, rwlock: &RwLock<T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    my_println_with_callsite!(callsite, "RwLock try-read-locking {:?}, this is before.", addr_of!(*rwlock));
}

pub fn this_is_our_rwlock_try_read_after_handle_function<T: ?Sized>(callsite: &str, rwlock: &RwLock<T>, ret: &mut TryLockResult<RwLockReadGuard<'_, T>>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let rwlock_addr = addr_of!(*rwlock);
    let rwlock_addr_u64 = rwlock_addr as *const () as u64;
    let locked = ret.is_ok();
    if locked {
        lock.get_locktree_current_thread().record_lock((rwlock_addr_u64, LockType::ReadLock));
        THREAD.with(|thread| {
            lock.lock_trace.write_record(&SyncRecord{
                timestamp: utils::get_timestamp_nanos(),
                thread_id: thread.id,
                memory_address: rwlock_addr_u64,
                operation_type: 3,
            }).unwrap();
        });
    }
    my_println_with_callsite!(callsite, "RwLock try-read-locking {:?}, this is after, ret addr {:?}, result {locked}", rwlock_addr, addr_of!(*ret));
}

pub fn this_is_our_rwlock_write_before_handle_function<T: ?Sized>(callsite: &str, rwlock: &RwLock<T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let rwlock_addr = addr_of!(*rwlock);
    let rwlock_addr_u64 = rwlock_addr as *const () as u64;
    my_println_with_callsite!(callsite, "RwLock write-locking {:?}, this is before.", addr_of!(*rwlock));
    THREAD.with(|thread| {
        lock.lock_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: rwlock_addr_u64,
            operation_type: 4,
        }).unwrap();
    });
    lock.get_locktree_current_thread().record_lock((rwlock_addr_u64, LockType::WriteLock));
}

pub fn this_is_our_rwlock_write_after_handle_function<T: ?Sized>(callsite: &str, rwlock: &RwLock<T>, ret: &mut LockResult<RwLockWriteGuard<'_, T>>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    my_println_with_callsite!(callsite, "RwLock write-locking {:?}, this is after, ret addr {:?}", addr_of!(*rwlock), addr_of!(*ret));
}

pub fn this_is_our_rwlock_try_write_before_handle_function<T: ?Sized>(callsite: &str, rwlock: &RwLock<T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    my_println_with_callsite!(callsite, "RwLock try-write-locking {:?}, this is before.", addr_of!(*rwlock));
}

pub fn this_is_our_rwlock_try_write_after_handle_function<T: ?Sized>(callsite: &str, rwlock: &RwLock<T>, ret: &mut TryLockResult<RwLockWriteGuard<'_, T>>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let rwlock_addr = addr_of!(*rwlock);
    let rwlock_addr_u64 = rwlock_addr as *const () as u64;
    let locked = ret.is_ok();
    if locked {
        lock.get_locktree_current_thread().record_lock((rwlock_addr_u64, LockType::WriteLock));
        THREAD.with(|thread| {
            lock.lock_trace.write_record(&SyncRecord{
                timestamp: utils::get_timestamp_nanos(),
                thread_id: thread.id,
                memory_address: rwlock_addr_u64,
                operation_type: 4,
            }).unwrap();
        });
    }
    my_println_with_callsite!(callsite, "RwLock try-write-locking {:?}, this is after, ret addr {:?}, result {locked}", addr_of!(*rwlock), addr_of!(*ret));
}

pub fn this_is_our_rwlock_readguard_drop_before_handle_function<'a, T: ?Sized + 'a>(callsite: &str, guard: &mut RwLockReadGuard<'a, T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let rwlock = exposed::rwlock_readguard_to_rwlock(guard);
    let rwlock_addr = addr_of!(*rwlock);
    let rwlock_addr_u64 = rwlock_addr as *const () as u64;
    my_println_with_callsite!(callsite, "RwLockReadGuard droping {:?}, inner RwLock {:?}, this is before", addr_of!(*guard), addr_of!(*rwlock));
    lock.get_locktree_current_thread().record_unlock((rwlock_addr_u64, LockType::ReadLock));
    THREAD.with(|thread| {
        lock.lock_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: rwlock_addr_u64,
            operation_type: 5,
        }).unwrap();
    });
}

pub fn this_is_our_rwlock_readguard_drop_after_handle_function<'a, T: ?Sized + 'a>(callsite: &str, guard: &mut RwLockReadGuard<'a, T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let rwlock = exposed::rwlock_readguard_to_rwlock(guard);
    my_println_with_callsite!(callsite, "RwLockReadGuard droping {:?}, inner RwLock {:?}, this is after", addr_of!(*guard), addr_of!(*rwlock));
}

pub fn this_is_our_rwlock_writeguard_drop_before_handle_function<'a, T: ?Sized + 'a>(callsite: &str, guard: &mut RwLockWriteGuard<'a, T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let rwlock = exposed::rwlock_writeguard_to_rwlock(guard);
    let rwlock_addr = addr_of!(*rwlock);
    let rwlock_addr_u64 = rwlock_addr as *const () as u64;
    my_println_with_callsite!(callsite, "RwLockWriteGuard droping {:?}, inner RwLock {:?}, this is before", addr_of!(*guard), addr_of!(*rwlock));
    lock.get_locktree_current_thread().record_unlock((rwlock_addr_u64, LockType::WriteLock));
    THREAD.with(|thread| {
        lock.lock_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: rwlock_addr_u64,
            operation_type: 6,
        }).unwrap();
    });
}

pub fn this_is_our_rwlock_writeguard_drop_after_handle_function<'a, T: ?Sized + 'a>(callsite: &str, guard: &mut RwLockWriteGuard<'a, T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let rwlock = exposed::rwlock_writeguard_to_rwlock(guard);
    my_println_with_callsite!(callsite, "RwLockWriteGuard droping {:?}, inner RwLock {:?}, this is after", addr_of!(*guard), addr_of!(*rwlock));
}


pub fn this_is_our_barrier_wait_before_handle_function(callsite: &str, barrier: &Barrier) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    my_println_with_callsite!(callsite, "Barrier waiting {:?}, this is before", addr_of!(*barrier));
    THREAD.with(|thread| {
        lock.sync_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: addr_of!(*barrier)as *const ()  as u64,
            operation_type: 7,
        }).unwrap();
    });
}

pub fn this_is_our_barrier_wait_after_handle_function(callsite: &str, barrier: &Barrier, ret: &mut BarrierWaitResult) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    my_println_with_callsite!(callsite, "Barrier waiting {:?}, this is after, is_leader:{}", addr_of!(*barrier), ret.is_leader());
}


pub fn this_is_our_condvar_wait_before_handle_function<'a, T>(callsite: &str, condvar: &Condvar, guard: &MutexGuard<'a, T>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Condvar waiting {:?} on mutexguard {:?} (unlocking mutex {mutex_addr:?}), this is before", addr_of!(*condvar), addr_of!(*guard));
    lock.get_locktree_current_thread().record_unlock((mutex_addr_u64, LockType::MutexLock));
}

pub fn this_is_our_condvar_wait_after_handle_function<'a, T>(callsite: &str, condvar: &Condvar, guard: &MutexGuard<'a, T>, ret: &mut LockResult<MutexGuard<'a, T>>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Condvar waiting {:?} om mutexguard {:?} (relocking mutex {mutex_addr:?}), this is after", addr_of!(*condvar), addr_of!(*guard));
    THREAD.with(|thread| {
        lock.sync_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: mutex_addr_u64,
            operation_type: 8,
        }).unwrap();
    });
    lock.get_locktree_current_thread().record_lock((mutex_addr_u64, LockType::MutexLock));
}


pub fn this_is_our_condvar_wait_timeout_before_handle_function<'a, T>(callsite: &str, condvar: &Condvar, guard: &MutexGuard<'a, T>, dur: &Duration) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Condvar timeout-waiting {:?} on mutexguard {:?} (unlocking mutex {mutex_addr:?}), this is before", addr_of!(*condvar), addr_of!(*guard));
    lock.get_locktree_current_thread().record_unlock((mutex_addr_u64, LockType::MutexLock));
}

pub fn this_is_our_condvar_wait_timeout_after_handle_function<'a, T>(callsite: &str, condvar: &Condvar, guard: &MutexGuard<'a, T>, dur: &Duration, ret: &mut LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Condvar timeout-waiting {:?} om mutexguard {:?} (relocking mutex {mutex_addr:?}), this is after", addr_of!(*condvar), addr_of!(*guard));
    THREAD.with(|thread| {
        lock.sync_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: mutex_addr_u64,
            operation_type: 8,
        }).unwrap();
    });
    lock.get_locktree_current_thread().record_lock((mutex_addr_u64, LockType::MutexLock));
}

pub fn this_is_our_condvar_wait_timeout_ms_before_handle_function<'a, T>(callsite: &str, condvar: &Condvar, guard: &MutexGuard<'a, T>, ms: &u32) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Condvar timeout-ms-waiting {:?} on mutexguard {:?} (unlocking mutex {mutex_addr:?}), this is before", addr_of!(*condvar), addr_of!(*guard));
    lock.get_locktree_current_thread().record_unlock((mutex_addr_u64, LockType::MutexLock));
}

pub fn this_is_our_condvar_wait_timeout_ms_after_handle_function<'a, T>(callsite: &str, condvar: &Condvar, guard: &MutexGuard<'a, T>, ms: &u32, ret: &mut LockResult<(MutexGuard<'a, T>, bool)>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Condvar timeout-ms-waiting {:?} om mutexguard {:?} (relocking mutex {mutex_addr:?}), this is after", addr_of!(*condvar), addr_of!(*guard));
    THREAD.with(|thread| {
        lock.sync_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: mutex_addr_u64,
            operation_type: 8,
        }).unwrap();
    });
    lock.get_locktree_current_thread().record_lock((mutex_addr_u64, LockType::MutexLock));
}

pub fn this_is_our_condvar_wait_while_before_handle_function<'a, T, F>(callsite: &str, condvar: &Condvar, guard: &MutexGuard<'a, T>, condition: F) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Condvar while-waiting {:?} on mutexguard {:?} (unlocking mutex {mutex_addr:?}), this is before", addr_of!(*condvar), addr_of!(*guard));
    lock.get_locktree_current_thread().record_unlock((mutex_addr_u64, LockType::MutexLock));
}

pub fn this_is_our_condvar_wait_while_after_handle_function<'a, T, F>(callsite: &str, condvar: &Condvar, guard: &MutexGuard<'a, T>, condition: F, ret: &mut LockResult<MutexGuard<'a, T>>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Condvar while-waiting {:?} om mutexguard {:?} (relocking mutex {mutex_addr:?}), this is after", addr_of!(*condvar), addr_of!(*guard));
    THREAD.with(|thread| {
        lock.sync_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: mutex_addr_u64,
            operation_type: 8,
        }).unwrap();
    });
    lock.get_locktree_current_thread().record_lock((mutex_addr_u64, LockType::MutexLock));
}

pub fn this_is_our_condvar_wait_timeout_while_before_handle_function<'a, T, F>(callsite: &str, condvar: &Condvar, guard: &MutexGuard<'a, T>, dur: &Duration, condition: F) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Condvar timeout-while-waiting {:?} on mutexguard {:?} (unlocking mutex {mutex_addr:?}), this is before", addr_of!(*condvar), addr_of!(*guard));
    lock.get_locktree_current_thread().record_unlock((mutex_addr_u64, LockType::MutexLock));
}

pub fn this_is_our_condvar_wait_timeout_while_after_handle_function<'a, T, F>(callsite: &str, condvar: &Condvar, guard: &MutexGuard<'a, T>, dur: &Duration, condition: F, ret: &mut LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)>) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    let mutex = exposed::mutexguard_to_mutex(guard);
    let mutex_addr = addr_of!(*mutex);
    let mutex_addr_u64 = mutex_addr as *const () as u64;
    my_println_with_callsite!(callsite, "Condvar timeout-while-waiting {:?} om mutexguard {:?} (relocking mutex {mutex_addr:?}), this is after", addr_of!(*condvar), addr_of!(*guard));
    THREAD.with(|thread| {
        lock.sync_trace.write_record(&SyncRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: mutex_addr_u64,
            operation_type: 8,
        }).unwrap();
    });
    lock.get_locktree_current_thread().record_lock((mutex_addr_u64, LockType::MutexLock));
}


pub fn this_is_our_mem_read_before_function(addr:usize) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    THREAD.with(|thread| {
        my_println!("mem read {addr} in thread {thread:?}");
        lock.memaccess_trace.write_record(&MemAccessRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: addr,
            operation_type: 0,
        }).unwrap();
    });
}


pub fn this_is_our_mem_write_before_function(addr:usize) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    THREAD.with(|thread| {
        my_println!("mem write {addr} in thread {thread:?}");
        lock.memaccess_trace.write_record(&MemAccessRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: addr,
            operation_type: 1,
        }).unwrap();
    });
}

pub fn this_is_our_mem_atomic_read_before_function(addr:usize) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    THREAD.with(|thread| {
        my_println!("mem atomic-read {addr} in thread {thread:?}");
        lock.memaccess_trace.write_record(&MemAccessRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: addr,
            operation_type: 0,
        }).unwrap();
    });
}

pub fn this_is_our_mem_atomic_write_before_function(addr:usize) {
    let mut lock = MONITOR_LOCK.lock().unwrap();
    THREAD.with(|thread| {
        my_println!("mem atomic-write {addr} in thread {thread:?}");
        lock.memaccess_trace.write_record(&MemAccessRecord{
            timestamp: utils::get_timestamp_nanos(),
            thread_id: thread.id,
            memory_address: addr,
            operation_type: 1,
        }).unwrap();
    });
}