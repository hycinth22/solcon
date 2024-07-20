use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write, Result};
use std::sync::atomic::{AtomicUsize, Ordering};

pub use memaccess_record::MemAccessRecord;
pub use memalloc_record::MemAllocRecord;
pub use sync_record::SyncRecord;

mod sync_record;
mod memaccess_record;
mod memalloc_record;

trait ToBytes{
    fn to_bytes(&self) -> Vec<u8>;
}

pub struct RecordWriter<Record: ToBytes + std::fmt::Display> {
    closing_need_flush_immediately: bool,
    writer: BufWriter<File>,
    records_written: AtomicUsize,
    record_type_marker: std::marker::PhantomData<Record>,
}

impl<Record: ToBytes + std::fmt::Display> RecordWriter<Record> {
    pub fn new(file: File) -> Self {
        Self {
            closing_need_flush_immediately: false,
            writer: BufWriter::new(file),
            records_written: AtomicUsize::new(0),
            record_type_marker: std::marker::PhantomData::default(),
        }
    }

    pub fn write_record_bin(&mut self, record: &Record) -> Result<()> {
        let bytes = record.to_bytes();
        self.writer.write_all(&bytes)?;
        self.records_written.fetch_add(1, Ordering::Relaxed);
        if self.closing_need_flush_immediately {
            self.flush()?;
        }
        Ok(())
    }
    
    pub fn write_record(&mut self, record: &Record) -> Result<()> {
        self.writer.write_fmt(format_args!("{}\n", record)).unwrap();
        self.records_written.fetch_add(1, Ordering::Relaxed);
        if self.closing_need_flush_immediately {
            self.flush()?;
        }
        Ok(())
    }

    pub fn records_written(&self) -> usize {
        self.records_written.load(Ordering::Relaxed)
    }

    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()
    }

    pub fn flush_and_close(&mut self) -> Result<()> {
        self.closing_need_flush_immediately = true;
        self.flush()
    }

}

pub fn create_lock_logger(prefix: &str) -> RecordWriter<SyncRecord> {
    let filename = format!("lock_{}.log", prefix);
    let file = OpenOptions::new().write(true).create(true).open(filename);
    RecordWriter::new(file.unwrap())
}

pub fn create_sync_logger(prefix: &str) -> RecordWriter<SyncRecord> {
    let filename = format!("sync_{}.log", prefix);
    let file = OpenOptions::new().write(true).create(true).open(filename);
    RecordWriter::new(file.unwrap())
}

pub fn create_memaccess_logger(prefix: &str) -> RecordWriter<MemAccessRecord> {
    let filename = format!("memaccess{}.log", prefix);
    let file = OpenOptions::new().write(true).create(true).open(filename);
    RecordWriter::new(file.unwrap())
}

pub fn create_memalloc_logger(prefix: &str) -> RecordWriter<MemAllocRecord> {
    let filename = format!("memalloc{}.log", prefix);
    let file = OpenOptions::new().write(true).create(true).open(filename);
    RecordWriter::new(file.unwrap())
}