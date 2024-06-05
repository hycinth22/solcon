use std::sync::Mutex;
use std::sync::RwLock;

fn main() {
    let mutex = Mutex::new(111);
    let guard = mutex.lock().unwrap();
    drop(guard);
    let guard = mutex.try_lock().unwrap();
    drop(guard);
    let rwlock = RwLock::new(222);
    let guard = rwlock.read().unwrap();
    drop(guard);
    let guard = rwlock.write().unwrap();
    drop(guard);
    let guard = rwlock.try_read().unwrap();
    drop(guard);
    let guard = rwlock.try_write().unwrap();
    drop(guard);
}
