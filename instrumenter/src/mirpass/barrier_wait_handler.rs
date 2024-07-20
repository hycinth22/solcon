use rustc_span::def_id::DefId;
use crate::{mirpass::FunctionCallInstrumenter, monitors_finder::MonitorsInfo};

#[derive(Default)]
pub struct BarrierWaitCallHandler<'pass>{
    __marker: std::marker::PhantomData<&'pass str>,
}

impl<'pass> FunctionCallInstrumenter<'_> for BarrierWaitCallHandler<'_> {
    #[inline]
    fn target_function(&self) -> &'static str {
        "std::sync::Barrier::wait"
    }

    #[inline]
    fn before_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> {
        let Some(our_func_def_id) = monitors.barrier_wait_before_fn else { warn!("monitors.barrier_wait_before_fn.is_none"); return None; };
        Some(our_func_def_id)
    }

    #[inline]
    fn after_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> {
        let Some(our_func_def_id) = monitors.barrier_wait_after_fn else { warn!("monitors.barrier_wait_after_fn.is_none"); return None; };
        Some(our_func_def_id)
    }
}

