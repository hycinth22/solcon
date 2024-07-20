
pub struct Reporter {
}

impl Reporter {
    pub fn new() -> Self {
        Self {
        }
    }

}

#[macro_export]
macro_rules! solcon_report {
    () => {
        eprint!("\n");
    };
    ($($arg:tt)*) => {{
        eprintln!($($arg)*);
    }};
}
