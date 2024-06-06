use rustc_span::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use tracing::{debug, info};
use crate::config;
use macro_monitors_finder::generate_impl_monitors_finder_from_monitors_info;

#[generate_impl_monitors_finder_from_monitors_info]
#[derive(Default, Debug)]
pub(crate) struct MonitorsInfo {
    #[monitor_defpath = "this_is_our_entry_fn_before_handle_function"]
    pub entry_fn_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_entry_fn_after_handle_function"]
    pub entry_fn_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_test_target_before_handle_function"]
    pub test_target_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_test_target_after_handle_function"]
    pub test_target_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_mutex_lock_before_handle_function"]
    pub mutex_lock_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_mutex_lock_after_handle_function"]
    pub mutex_lock_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_mutex_try_lock_before_handle_function"]
    pub mutex_try_lock_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_mutex_try_lock_after_handle_function"]
    pub mutex_try_lock_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_mutexguard_drop_before_handle_function"]
    pub mutexguard_drop_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_mutexguard_drop_after_handle_function"]
    pub mutexguard_drop_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_rwlock_read_before_handle_function"]
    pub rwlock_read_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_rwlock_read_after_handle_function"]
    pub rwlock_read_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_rwlock_write_before_handle_function"]
    pub rwlock_write_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_rwlock_write_after_handle_function"]
    pub rwlock_write_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_rwlock_try_read_before_handle_function"]
    pub rwlock_try_read_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_rwlock_try_read_after_handle_function"]
    pub rwlock_try_read_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_rwlock_try_write_before_handle_function"]
    pub rwlock_try_write_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_rwlock_try_write_after_handle_function"]
    pub rwlock_try_write_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_rwlock_readguard_drop_before_handle_function"]
    pub rwlock_readguard_drop_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_rwlock_readguard_drop_after_handle_function"]
    pub rwlock_readguard_drop_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_rwlock_writeguard_drop_before_handle_function"]
    pub rwlock_writeguard_drop_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_rwlock_writeguard_drop_after_handle_function"]
    pub rwlock_writeguard_drop_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_barrier_wait_before_handle_function"]
    pub barrier_wait_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_barrier_wait_after_handle_function"]
    pub barrier_wait_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_condvar_wait_before_handle_function"]
    pub condvar_wait_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_condvar_wait_after_handle_function"]
    pub condvar_wait_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_condvar_wait_timeout_before_handle_function"]
    pub condvar_wait_timeout_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_condvar_wait_timeout_after_handle_function"]
    pub condvar_wait_timeout_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_condvar_wait_timeout_ms_before_handle_function"]
    pub condvar_wait_timeout_ms_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_condvar_wait_timeout_ms_after_handle_function"]
    pub condvar_wait_timeout_ms_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_condvar_wait_while_before_handle_function"]
    pub condvar_wait_while_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_condvar_wait_while_after_handle_function"]
    pub condvar_wait_while_after_fn: Option<DefId>,

    #[monitor_defpath = "this_is_our_condvar_wait_timeout_while_before_handle_function"]
    pub condvar_wait_timeout_while_before_fn: Option<DefId>,
    #[monitor_defpath = "this_is_our_condvar_wait_timeout_while_after_handle_function"]
    pub condvar_wait_timeout_while_after_fn: Option<DefId>,
}

pub trait MonitorsFinder {
    fn try_match_with_our_function(&mut self, tcx: TyCtxt<'_>, fn_def_id: &DefId) -> bool;
}
