use std::sync::Mutex;
use std::thread;

struct A {
    a: i32,
    b: i32,
}

fn main() {
    println!("create mutex");
    let mutex = Mutex::new(A{
        a: 1,
        b: 2,
    });
    println!("test mutex lock1");
    let mut guard = mutex.lock().unwrap();
    guard.a = 3;
    guard.b = 4;
    anotherfn(&guard.a);
    anotherfn2(&mut guard.a);
    //guard.b = guard.a;
    drop(guard);
    thread::spawn(move || {
	    println!("test mutex lock2");
	    let guard = mutex.lock().unwrap();
	    drop(guard);
    }).join();
}

fn anotherfn(x: &i32) {
    println!("anotherfn");
    let y = *x;
}

fn anotherfn2(x: &mut i32) {
    println!("anotherfn2");
    *x = 1;
}