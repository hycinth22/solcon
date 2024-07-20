use rustc_span::def_id::DefId;
use crate::monitors_finder::MonitorsInfo;

#[derive(Default)]
pub struct MutexGuardDropInstrumenter{}

impl crate::mirpass::ObjectDropInstrumenter for MutexGuardDropInstrumenter {
    #[inline]
    fn target_ty(&self) -> &'static str {
        "std::sync::MutexGuard"
    }
    #[inline]
    fn before_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> {
        monitors.mutexguard_drop_before_fn
    }
    #[inline]
    fn after_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> {
        monitors.mutexguard_drop_after_fn
    }
}