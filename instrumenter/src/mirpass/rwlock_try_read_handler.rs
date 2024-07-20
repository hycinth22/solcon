use rustc_span::def_id::DefId;
use crate::{mirpass::FunctionCallInstrumenter, monitors_finder::MonitorsInfo};

#[derive(Default)]
pub struct RwLockTryReadCallHandler<'pass>{
    __marker: std::marker::PhantomData<&'pass str>,
}

impl<'pass> FunctionCallInstrumenter<'_> for RwLockTryReadCallHandler<'_> {
    #[inline]
    fn target_function(&self) -> &'static str {
        "std::sync::RwLock::<T>::try_read"
    }

    #[inline]
    fn before_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> {
        let Some(our_func_def_id) = monitors.rwlock_try_read_before_fn else { warn!("monitors.rwlock_try_read_before_fn.is_none"); return None; };
        Some(our_func_def_id)
    }

    #[inline]
    fn after_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> {
        let Some(our_func_def_id) = monitors.rwlock_try_read_after_fn else { warn!("monitors.rwlock_try_read_after_fn.is_none"); return None; };
        Some(our_func_def_id)
    }
}

