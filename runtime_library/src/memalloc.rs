use tracking_allocator::{
    AllocationGroupId, AllocationGroupToken, AllocationRegistry, AllocationTracker, Allocator,
};

use std::{alloc::{GlobalAlloc, System}, sync::{Arc, Mutex}};

use crate::{recorder::{self, create_memalloc_logger, MemAllocRecord, RecordWriter}, utils};

// This is where we actually set the global allocator to be the shim allocator implementation from `tracking_allocator`.
// This allocator is purely a facade to the logic provided by the crate, which is controlled by setting a global tracker
// and registering allocation groups.  All of that is covered below.
//
// As well, you can see here that we're wrapping the system allocator.  If you want, you can construct `Allocator` by
// wrapping another allocator that implements `GlobalAlloc`.  Since this is a static, you need a way to construct ther
// allocator to be wrapped in a const fashion, but it _is_ possible.
pub type GlobalSystemAllocatorType = std::alloc::System;
pub static GLOBAL_SYSTEM_ALLOCATOR : GlobalSystemAllocatorType = GlobalSystemAllocatorType{};
#[global_allocator]
static GLOBAL: Allocator<GlobalSystemAllocatorType> = Allocator::from_allocator(GLOBAL_SYSTEM_ALLOCATOR);


struct StdoutTracker{
    monitor_locked_info: Arc<Mutex<crate::MonitorLockedInformation>, GlobalSystemAllocatorType>,
    recorder: Arc<Mutex< recorder::RecordWriter<MemAllocRecord> >, GlobalSystemAllocatorType>
}
// This is our tracker implementation.  You will always need to create an implementation of `AllocationTracker` in order
// to actually handle allocation events.  The interface is straightforward: you're notified when an allocation occurs,
// and when a deallocation occurs.
impl AllocationTracker for StdoutTracker {
    fn allocated(
        &self,
        addr: usize,
        object_size: usize,
        wrapped_size: usize,
        group_id: AllocationGroupId,
    ) {
        let timestamp = utils::get_timestamp_nanos();
        // Allocations have all the pertinent information upfront, which you may or may not want to store for further
        // analysis. Notably, deallocations also know how large they are, and what group ID they came from, so you
        // typically don't have to store much data for correlating deallocations with their original allocation.
        println!(
            "allocation -> addr=0x{:0x} object_size={} wrapped_size={} group_id={:?}",
            addr, object_size, wrapped_size, group_id
        );
        self.recorder.try_lock().unwrap().write_record(&MemAllocRecord {
            timestamp,
            addr,
            len: object_size,
            operation_type: 0,
        }).unwrap();
    }

    fn deallocated(
        &self,
        addr: usize,
        object_size: usize,
        wrapped_size: usize,
        source_group_id: AllocationGroupId,
        current_group_id: AllocationGroupId,
    ) {
        let timestamp = utils::get_timestamp_nanos();
        // When a deallocation occurs, as mentioned above, you have full access to the address, size of the allocation,
        // as well as the group ID the allocation was made under _and_ the active allocation group ID.
        //
        // This can be useful beyond just the obvious "track how many current bytes are allocated by the group", instead
        // going further to see the chain of where allocations end up, and so on.
        println!(
            "deallocation -> addr=0x{:0x} object_size={} wrapped_size={} source_group_id={:?} current_group_id={:?}",
            addr, object_size, wrapped_size, source_group_id, current_group_id
        );
        self.recorder.try_lock().unwrap().write_record(&MemAllocRecord {
            timestamp,
            addr,
            len: object_size,
            operation_type: 1,
        }).unwrap();
    }
}

pub fn register_memalloc_tracker(monitor_locked_info: Arc<Mutex<crate::MonitorLockedInformation>, GlobalSystemAllocatorType>, recorder: Arc<Mutex< recorder::RecordWriter<MemAllocRecord> >, GlobalSystemAllocatorType> ) {
    // Create and set our allocation tracker.  Even with the tracker set, we're still not tracking allocations yet.  We
    // need to enable tracking explicitly.
    let _ = AllocationRegistry::set_global_tracker(StdoutTracker{
        monitor_locked_info,
        recorder,
    })
        .expect("no other global tracker should be set yet");
    AllocationRegistry::enable_tracking();
}


// use std::alloc::{GlobalAlloc, Layout, System};
// use std::collections::BTreeMap;
// use std::sync::{Mutex, Arc};

// use once_cell::sync::Lazy;

// use crate::utils::get_timestamp_nanos;

// pub struct AllocatorLog {
//     pub timestamp: u128,
//     pub addr: usize,
//     pub size: usize,
//     pub is_alloc: bool,
// }

// pub struct AllocatorLogger {
//     allocations: Arc<Mutex<Vec<AllocatorLog, System>>, System>,
// }

// impl AllocatorLogger {
//     fn alloc(&self, ptr: *mut u8, layout: Layout) {
//         if !ptr.is_null() {
//             let ts = get_timestamp_nanos();
//             let addr = ptr as usize;
//             let len = layout.size();
//             let mut allocations = self.allocations.lock().unwrap();
//             allocations.push(AllocatorLog {
//                 timestamp: ts,
//                 addr,
//                 size: len,
//                 is_alloc: true,
//             });
//             //println!("Allocated {} bytes at {:p}", layout.size(), ptr);
//         }
//     }

//     fn dealloc(&self, ptr: *mut u8, layout: Layout) {
//         if !ptr.is_null() {
//             let ts = get_timestamp_nanos();
//             let addr = ptr as usize;
//             let len = layout.size();
//             let mut allocations = self.allocations.lock().unwrap();
//             allocations.push(AllocatorLog {
//                 timestamp: ts,
//                 addr,
//                 size: len,
//                 is_alloc: false,
//             });
//             //println!("Deallocated {} bytes from {:p}", layout.size(), ptr);
//         }
//     }
// }

// pub struct LoggingAllocator {
//     pub logger: Lazy<AllocatorLogger>,
// }

// // 实现 GlobalAlloc trait
// unsafe impl GlobalAlloc for LoggingAllocator {
//     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
//         let ptr = System.alloc(layout);
//         self.logger.alloc(ptr, layout);
//         ptr
//     }

//     unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
//         self.logger.dealloc(ptr, layout);
//         System.dealloc(ptr, layout)
//     }
// }


// // #[global_allocator]
// pub static GLOBAL_ALLOCATOR: LoggingAllocator = LoggingAllocator{
//     logger: Lazy::new(|| AllocatorLogger {
//         allocations: Arc::new_in(Mutex::new(Vec::new_in(System)), System),
//     })
// };