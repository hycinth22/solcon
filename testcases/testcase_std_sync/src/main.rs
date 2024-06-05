use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::Barrier;
use std::sync::Condvar;
use std::sync::Arc;
use std::thread;

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
    let barrier = Barrier::new(1);
    barrier.wait();
    let cm_pair = Arc::new((Condvar::new(), Mutex::new(())));
    let cm_pair2 = Arc::clone(&cm_pair);
    let (condvar, mutex) = &*cm_pair;
    let gurad = mutex.lock().unwrap();
    thread::spawn(move || {
        let (condvar, mutex) = &*cm_pair2;
        drop(mutex.lock());
        condvar.notify_one();
    });
    let guard = condvar.wait(gurad);
}
