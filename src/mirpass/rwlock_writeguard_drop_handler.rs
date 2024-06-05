use rustc_span::def_id::DefId;
use crate::monitors_finder::MonitorsInfo;

#[derive(Default)]
pub struct RwLockWriteGuardDropInstrumenter{}

impl crate::mirpass::ObjectDropInstrumenter for RwLockWriteGuardDropInstrumenter {
    #[inline]
    fn target_ty(&self) -> &'static str {
        "std::sync::RwLockWriteGuard"
    }
    #[inline]
    fn before_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> {
        monitors.rwlock_writeguard_drop_before_fn
    }
    #[inline]
    fn after_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> {
        monitors.rwlock_writeguard_drop_after_fn
    }
}