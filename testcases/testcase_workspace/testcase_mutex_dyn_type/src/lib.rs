use std::sync::Mutex;

struct S{
   m: Box<Mutex<dyn Send + 'static>>,
}

pub fn ff() {
    println!("hello from testcase_mutex_dyn_type");
    let s = S{
         m: Box::new(Mutex::new(123)),
    };
    let mut g = s.m.lock().unwrap();
    println!("bye from testcase_mutex_dyn_type");
}
