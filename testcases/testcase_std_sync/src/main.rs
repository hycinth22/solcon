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
    println!("create mutex");
    let mutex = Mutex::new(111);
    println!("test mutex lock");
    let guard = mutex.lock().unwrap();
    drop(guard);
    println!("test mutex try_lock");
    let guard = mutex.try_lock().unwrap();
    drop(guard);
    println!("create RwLock");
    let rwlock = RwLock::new(222);
    println!("test RwLock read");
    let guard = rwlock.read().unwrap();
    drop(guard);
    println!("test RwLock write");
    let guard = rwlock.write().unwrap();
    drop(guard);
    println!("test RwLock try_read");
    let guard = rwlock.try_read().unwrap();
    drop(guard);
    println!("test RwLock try_write");
    let guard = rwlock.try_write().unwrap();
    drop(guard);
    println!("create Barrier");
    let barrier = Barrier::new(1);
    println!("test Barrier wait");
    barrier.wait();

    testconvar();

    test_multimutex();
}

fn testconvar() {
    println!("create (Condvar, Mutex) pair");
    let cm_pair = Arc::new((Condvar::new(), Mutex::new(false)));

    println!("locking Condvar's Mutex");
    let (condvar, mutex) = &*cm_pair;
    let guard = mutex.lock().unwrap();

    println!("test Condvar wait");
    spwan_notify_thread(Arc::clone(&cm_pair));
    let guard = condvar.wait(guard).unwrap();

    println!("test Condvar wait_timeout");
    spwan_notify_thread(Arc::clone(&cm_pair));
    let (guard, _timeoutresult) = condvar.wait_timeout(guard, Duration::MAX).unwrap();

    println!("test Condvar wait_timeout_ms");
    spwan_notify_thread(Arc::clone(&cm_pair));
    let (mut guard, _timeoutresult) = condvar.wait_timeout_ms(guard, u32::MAX).unwrap();

    println!("test Condvar wait_while");
    *guard = false;
    spwan_notify_thread(Arc::clone(&cm_pair));
    let mut guard = condvar.wait_while(guard, |val| *val).unwrap();

    println!("test Condvar wait_timeout_while");
    *guard = false;
    spwan_notify_thread(Arc::clone(&cm_pair));
    let guard = condvar.wait_timeout_while(guard, Duration::MAX, |val| *val).unwrap();
}

fn test_multimutex() {
    let a = Mutex::new(1);
    let b = Mutex::new(2);
    let c = Mutex::new(3);
    let g1 = a.lock();
    let g2 = b.lock();
    let g3 = c.lock();
}