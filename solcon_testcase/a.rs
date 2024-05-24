// 在 Cargo.toml 中添加 log crate
// [dependencies]
// log = "0.4"

// main.rs 或者你的源码文件中
#[macro_use]
extern crate log;

// 定义一个宏来实现在函数调用前后打印日志
macro_rules! logged_call {
    // $func:expr 表示传入的是一个表达式
    ($func:expr) => {{
        info!("Calling function: {}", stringify!($func)); // 在调用前输出日志
        let result = $func; // 调用函数
        info!("Function call result: {:?}", result); // 在调用后输出日志
        result // 返回调用结果
    }};
}

// 你的函数
fn my_function() -> i32 {
    42
}

fn main() {
    // 初始化日志记录器
    env_logger::init();

    // 使用宏来调用函数并输出日志
    let result = logged_call!(my_function());
    println!("Result: {}", result);
}
