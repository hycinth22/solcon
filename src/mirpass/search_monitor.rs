use rustc_hir::def_id::DefId;
use rustc_middle::{mir::Body, ty::TyCtxt};

#[derive(Default, Debug)]
pub(crate) struct PreScanInfo {
    pub test_target_before_fn: Option<DefId>,
    pub test_target_after_fn: Option<DefId>,
    
    pub mutex_lock_before_fn: Option<DefId>,
    pub mutex_lock_after_fn: Option<DefId>,
}

pub(crate) fn try_match_with_our_function<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx Body<'tcx>, info: &mut PreScanInfo)  {
    let def_id: DefId = body.source.def_id();
    let fn_defpath_str = tcx.def_path_str(def_id);
    println!("try_match_with_our_function {}", fn_defpath_str);
    match fn_defpath_str.as_str() {
        "this_is_our_monitor_function::this_is_our_test_target_before_handle_function" => {
            info.test_target_before_fn = Some(def_id);
            println!("configure info.test_target_before_fn");
        }
        "this_is_our_monitor_function::this_is_our_test_target_after_handle_function" => {
            info.test_target_after_fn = Some(def_id);
            println!("configure info.test_target_after_fn");
        }
        "this_is_our_monitor_function::this_is_our_mutex_lock_before_handle_function" => {
            info.mutex_lock_before_fn = Some(def_id);
            println!("configure info.mutex_lock_before_fn");
        }
        "this_is_our_monitor_function::this_is_our_mutex_lock_after_handle_function" => {
            info.mutex_lock_after_fn = Some(def_id);
            println!("configure info.mutex_lock_after_fn");
        }
        &_=> {}
    }
}
