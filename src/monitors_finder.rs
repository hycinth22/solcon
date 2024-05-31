use rustc_span::def_id::{DefId, CrateNum};
use rustc_middle::{mir::Body, ty::TyCtxt};
use tracing::{debug, info};
use crate::config;
use macro_monitors_finder::generate_impl_monitors_finder_from_monitors_info;

#[generate_impl_monitors_finder_from_monitors_info]
#[derive(Default, Debug)]
pub(crate) struct MonitorsInfo {
    #[monitor_defpath = "this_is_our_test_target_before_handle_function"]
    pub test_target_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_test_target_after_handle_function"]
    pub test_target_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_mutex_lock_before_handle_function"]
    pub mutex_lock_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_mutex_lock_after_handle_function"]
    pub mutex_lock_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_mutexguard_drop_before_handle_function"]
    pub mutexguard_drop_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_mutexguard_drop_after_handle_function"]
    pub mutexguard_drop_after_fn: Option<DefId>,
}

pub trait MonitorsFinder {
    fn try_match_with_our_function(&mut self, tcx: TyCtxt<'_>, fn_def_id: &DefId) -> bool;
}
