use std::sync::Mutex;
use std::sync::MutexGuard;
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
    test_conflict_lock();
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
    let mut guard = condvar.wait_while(guard, |val| !*val).unwrap();

    println!("test Condvar wait_timeout_while");
    *guard = false;
    spwan_notify_thread(Arc::clone(&cm_pair));
    let guard = condvar.wait_timeout_while(guard, Duration::MAX, |val| !*val).unwrap();
}

fn test_multimutex() {
    let a = Mutex::new(1);
    let b = Mutex::new(2);
    let c = Mutex::new(3);
    let d = RwLock::new(4);
    {
        let g1 = a.lock().unwrap();
        let g2 = b.lock().unwrap();
        let g3 = c.lock().unwrap();
        let g4 = d.read().unwrap();
        drop(g4);
        drop(g3);
        drop(g2);
        drop(g1);
    }
    {
        let g5 = d.write().unwrap();
        drop(g5);
    }
    {
        let g6 = d.read().unwrap();
        let g7 = d.read().unwrap();
        drop(g6);
        drop(g7);
    }
    {
        let g1 = a.lock().unwrap();
        let g2 = b.lock().unwrap();
        let g3 = c.lock().unwrap();
        let g4 = d.read().unwrap();
        drop(g4);
        let g5 = d.write().unwrap();
        drop(g5);
    }
    {
        let t = Mutex::new(5);
        thread::spawn(move || {
            let g1 = t.lock().unwrap();
            let g1 = t.lock().unwrap();
            unreachable!("should deadlock before here");
        });
    }
    {
        let t = Arc::new(RwLock::new(6));
        let t_2 = Arc::clone(&t);
        let barrier1_1 = Arc::new(Barrier::new(2));
        let barrier1_2 = Arc::clone(&barrier1_1);
        let barrier2_1 = Arc::new(Barrier::new(2));
        let barrier2_2 = Arc::new(Barrier::new(2));
        thread::spawn(move || {
            let g1 = t.write().unwrap();
            barrier1_1.wait();
            barrier2_1.wait();
        });
        thread::spawn(move || {
            let g1 = t_2.read().unwrap();
            barrier1_2.wait();
            let g1 = t_2.read().unwrap();
            barrier2_2.wait();
            unreachable!("should deadlock before here");
        });
    }
    {
        let t = RwLock::new(7);
        thread::spawn(move || {
            let g1 = t.write().unwrap();
            let g1 = t.write().unwrap();
            unreachable!("should deadlock before here");
        });
    }
    {
        // test drop in random order
        let g1 = a.lock().unwrap();
        let g2 = b.lock().unwrap();
        let g3 = c.lock().unwrap();
        let g4 = d.read().unwrap();
        test_moveguard(g2);
        drop(g1);
    }
}

fn test_moveguard<T>(guard: MutexGuard<T>) {}

fn test_conflict_lock() {
    {
        println!("test confilct lock");
        let m1_arc = Arc::new(Mutex::new(7));
        let m1_arc2 = Arc::clone(&m1_arc);
        let m2_arc = Arc::new(Mutex::new(8));
        let m2_arc2 = Arc::clone(&m2_arc);
        let barrier_arc1 = Arc::new(Barrier::new(2));
        let barrier_arc2 = Arc::clone(&barrier_arc1);
        let barrier_arc3 = Arc::clone(&barrier_arc1);
        // test conlict lock order
        thread::spawn(move || {
            let g1 = m1_arc.lock().unwrap();
            barrier_arc1.wait();
            let g2 = m2_arc.lock().unwrap();
            unreachable!("should deadlock before here");
        });
        thread::spawn(move || {
            let g2 = m2_arc2.lock().unwrap();
            barrier_arc2.wait();
            let g1 = m1_arc2.lock().unwrap();
            unreachable!("should deadlock before here");
        });
        thread::sleep(Duration::from_millis(1000));
    }
    {
        println!("test confilct lock");
        let m1_arc = Arc::new(RwLock::new(7));
        let m1_arc2 = Arc::clone(&m1_arc);
        let m2_arc = Arc::new(RwLock::new(8));
        let m2_arc2 = Arc::clone(&m2_arc);
        let barrier_arc1 = Arc::new(Barrier::new(2));
        let barrier_arc2 = Arc::clone(&barrier_arc1);
        let barrier_arc3 = Arc::clone(&barrier_arc1);
        // test conlict lock order
        thread::spawn(move || {
            let g1 = m1_arc.write().unwrap();
            barrier_arc1.wait();
            let g2 = m2_arc.write().unwrap();
            unreachable!("should deadlock before here");
        });
        thread::spawn(move || {
            let g2 = m2_arc2.write().unwrap();
            barrier_arc2.wait();
            let g1 = m1_arc2.write().unwrap();
            unreachable!("should deadlock before here");
        });
        thread::sleep(Duration::from_millis(1000));
    }
    {
        println!("test confilct lock");
        let m1_arc = Arc::new(Mutex::new(7));
        let m1_arc2 = Arc::clone(&m1_arc);
        let m2_arc = Arc::new(RwLock::new(8));
        let m2_arc2 = Arc::clone(&m2_arc);
        let barrier_arc1 = Arc::new(Barrier::new(2));
        let barrier_arc2 = Arc::clone(&barrier_arc1);
        let barrier_arc3 = Arc::clone(&barrier_arc1);
        // test conlict lock order
        thread::spawn(move || {
            let g1 = m1_arc.lock().unwrap();
            barrier_arc1.wait();
            let g2 = m2_arc.write().unwrap();
            unreachable!("should deadlock before here");
        });
        thread::spawn(move || {
            let g2 = m2_arc2.write().unwrap();
            barrier_arc2.wait();
            let g1 = m1_arc2.lock().unwrap();
            unreachable!("should deadlock before here");
        });
        thread::sleep(Duration::from_millis(1000));
    }
}