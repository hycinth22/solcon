// this is a handler for test & templete purpose

use rustc_hir::def_id::DefId;

use crate::monitors_finder::MonitorsInfo;
use crate::mirpass::FunctionCallInstrumenter;


#[derive(Default)]
pub struct TestTargetCallHandler<'pass>{
    __marker: std::marker::PhantomData<&'pass str>,
}

impl<'pass> FunctionCallInstrumenter<'_> for TestTargetCallHandler<'_>{
    #[inline]
    fn target_function(&self) -> &'static str {
        "this_is_our_test_target_mod::this_is_our_test_target_function"
    }
    #[inline]
    fn before_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> { 
        let Some(our_func_def_id) = monitors.test_target_before_fn else { warn!("monitors.test_target_before_fn.is_none"); return None; };
        Some(our_func_def_id)
    }
    #[inline]
    fn after_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> { 
        let Some(our_func_def_id) = monitors.test_target_after_fn else { warn!("monitors.test_target_after_fn.is_none"); return None; };
        Some(our_func_def_id)
    }
}
