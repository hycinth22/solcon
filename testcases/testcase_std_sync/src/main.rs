use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::Barrier;
use std::sync::Condvar;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn spwan_notify_thread(cm_pair2: Arc<(Condvar, Mutex<bool>)> ) {
    thread::spawn(move || {
        let (condvar, mutex) = &*cm_pair2;
        let mut guard = mutex.lock().unwrap();
        *guard = true;
        drop(guard);
        condvar.notify_one();
    });
}


#[allow(deprecated)]
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

    testconvar();
}

fn testconvar() {

    let cm_pair = Arc::new((Condvar::new(), Mutex::new(false)));

    let (condvar, mutex) = &*cm_pair;
    let guard = mutex.lock().unwrap();

    spwan_notify_thread(Arc::clone(&cm_pair));
    let guard = condvar.wait(guard).unwrap();

    spwan_notify_thread(Arc::clone(&cm_pair));
    let (guard, _timeoutresult) = condvar.wait_timeout(guard, Duration::MAX).unwrap();

    spwan_notify_thread(Arc::clone(&cm_pair));
    let (mut guard, _timeoutresult) = condvar.wait_timeout_ms(guard, u32::MAX).unwrap();

    *guard = false;
    spwan_notify_thread(Arc::clone(&cm_pair));
    let mut guard = condvar.wait_while(guard, |val| *val).unwrap();

    *guard = false;
    spwan_notify_thread(Arc::clone(&cm_pair));
    let guard = condvar.wait_timeout_while(guard, Duration::MAX, |val| *val).unwrap();
}