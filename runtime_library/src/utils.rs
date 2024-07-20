use std::thread;
use std::time::SystemTime;

pub type ThreadId = u64;
#[derive(Debug)]
pub struct ThreadInfo {
    pub id: ThreadId,
    pub name: ThreadName,
}
#[derive(Debug)]
pub struct ThreadName (Option<String>);

impl std::fmt::Display for ThreadName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self.0 {
            Some(name) => write!(f, "\"{}\"", name),
            None => write!(f, "None-name"),
        }
    }
}

pub fn get_current_thread_info() -> ThreadInfo {
    let c = thread::current();
    let id = c.id().as_u64().get();
    let name = c.name().map(str::to_owned);
    ThreadInfo{id, name: ThreadName(name)}
}

pub fn get_timestamp_nanos() -> u128 {
    let now = SystemTime::now();
    let duruation = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    duruation.as_nanos()
}

pub fn get_full_std_stacktrace_str() -> String {
    std::backtrace::Backtrace::force_capture().to_string()
}

pub fn get_full_stacktrace_str(skip_cnt: usize) -> String {
    use backtrace::*;
    let bt = Backtrace::new();
    let mut frames:Vec<BacktraceFrame> = bt.into();
    let bt:Backtrace = frames.into_iter().skip(skip_cnt+1).collect::<Vec<_>>().into();
    format!("{bt:?}")
}

pub fn get_full_stacktrace_ip_str(skip_cnt: usize) -> String {
    use backtrace::*;
    let bt = Backtrace::new_unresolved();
    let frames:Vec<BacktraceFrame> = bt.into();
    frames.iter().skip(skip_cnt+1).map(|frame| format!("{:?}", frame.ip()) ).collect::<Vec<_>>().join(",")
}