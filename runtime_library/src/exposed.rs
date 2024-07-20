use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, LockResult, TryLockResult};
use std::sync::{Barrier, BarrierWaitResult};
use std::sync::{Condvar, WaitTimeoutResult};
use std::mem::transmute_copy;

// from std::sync::MutexGuard
pub(crate) struct ExposedMutexGuard<'a, T: ?Sized + 'a> {
    pub(crate) lock: &'a Mutex<T>,
    pub(crate) poison: ExposedPoisonGuard,
}

// from std::sync::poison::Guard
#[derive(Clone)]
pub struct ExposedPoisonGuard {
    #[cfg(panic = "unwind")]
    pub(crate) panicking: bool,
}

// from rust/library/std/src/sys/sync/rwlock/futex.rs
pub struct SysRwLock {
    pub(crate) state: std::sync::atomic::AtomicU32,
    pub(crate) writer_notify: std::sync::atomic::AtomicU32,
}

// from std::sync::RwLockReadGuard
pub struct ExposedRwLockReadGuard<'a, T: ?Sized + 'a> {
    pub(crate) data: std::ptr::NonNull<T>,
    pub(crate) inner_lock: &'a SysRwLock,
}


// from std::sync::RwLockWriteGuard
pub struct ExposedRwLockWriteGuard<'a, T: ?Sized + 'a> {
    pub(crate) lock: &'a std::sync::RwLock<T>,
    pub(crate) poison: ExposedPoisonGuard,
}

pub(crate) fn mutexguard_to_mutex<'a, T: ?Sized + 'a>(guard: &MutexGuard<'a, T>) -> &'a Mutex<T> {
    let exposed : ExposedMutexGuard<T> = unsafe {transmute_copy(guard)};
    exposed.lock
}

pub(crate) fn rwlock_readguard_to_rwlock<'a, T: ?Sized + 'a>(guard: &RwLockReadGuard<'a, T>) -> &'a SysRwLock {
    let exposed : ExposedRwLockReadGuard<T> = unsafe {transmute_copy(guard)};
    exposed.inner_lock
}

pub(crate) fn rwlock_writeguard_to_rwlock<'a, T: ?Sized + 'a>(guard: &RwLockWriteGuard<'a, T>) -> &'a std::sync::RwLock<T> {
    let exposed : ExposedRwLockWriteGuard<T> = unsafe {transmute_copy(guard)};
    exposed.lock
}