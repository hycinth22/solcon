use rustc_span::def_id::{DefId, CrateNum};
use rustc_middle::{mir::Body, ty::TyCtxt};
use tracing::{debug, info};
use crate::config;

#[derive(Default, Debug)]
pub(crate) struct MonitorsInfo {
    pub test_target_before_fn: Option<DefId>,
    pub test_target_after_fn: Option<DefId>,
    
    pub mutex_lock_before_fn: Option<DefId>,
    pub mutex_lock_after_fn: Option<DefId>,
}

pub(crate) fn try_match_with_our_function<'tcx>(tcx: TyCtxt<'tcx>, fn_def_id: &DefId, info: &mut MonitorsInfo)  {
    let def_id = fn_def_id.clone();
    let fn_defpath_str = tcx.def_path_str(def_id);
    info!("try_match_with_our_function {}", fn_defpath_str);
    let prefix = format!("{}::", config::MONITORS_LIB_CRATE_NAME);
    let Some(fn_defpath_str) = fn_defpath_str.strip_prefix(&prefix) else {
        return;
    };
    match fn_defpath_str {
        "this_is_our_test_target_before_handle_function" => {
            info.test_target_before_fn = Some(def_id);
            info!("configure info.test_target_before_fn");
        }
        "this_is_our_test_target_after_handle_function" => {
            info.test_target_after_fn = Some(def_id);
            info!("configure info.test_target_after_fn");
        }
        "this_is_our_mutex_lock_before_handle_function" => {
            info.mutex_lock_before_fn = Some(def_id);
            info!("configure info.mutex_lock_before_fn");
        }
        "this_is_our_mutex_lock_after_handle_function" => {
            info.mutex_lock_after_fn = Some(def_id);
            info!("configure info.mutex_lock_after_fn");
        }
        &_=> {}
    }
}
