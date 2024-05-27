use env_logger::{Builder, Target};
use log::Record;
use log::Log;

pub fn ff() {
     println!("hello from testcase_env_logger");
     let mut b = env_logger::builder();
     b.target(Target::Stdout);
     let l = b.build();
     let record = Record::builder()
                .args(format_args!("Error!"))
                .target("myApp")
                .file(Some("server.rs"))
                .line(Some(144))
                .module_path(Some("server"))
                .build();
     l.log(&record);
     println!("bye from testcase_env_logger");
}
