
use std::sync::{Mutex, LockResult, MutexGuard};
use std::ptr::addr_of;
use std::thread;
use std::sync::Barrier;
use std::time::SystemTime;

fn main() {
    let b1 = Barrier::new(2);
    let b2 = Barrier::new(2);
    thread::scope(|s| {
        s.spawn(|| {
            b1.wait();
            let b1 = Box::new(1);
            let addr1 = addr_of!(b1);
            println!("t1'box addr: {addr1:?}");
        });
        s.spawn(|| {
            b2.wait();
            let b2 = Box::new(2);
            let addr2 = addr_of!(b2);
            println!("t2'box addr: {addr2:?}");
        });
        let time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as u64;
        if time % 2 == 0 {
            b1.wait();
            b2.wait();
        } else {
            b2.wait();
            b1.wait();
        }

    });
}
