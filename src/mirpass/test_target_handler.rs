// this is a handler for test & templete purpose

use std::collections::HashMap;

use rustc_hir::def_id::DefId;
use rustc_middle::mir::{self, BasicBlock, BasicBlockData, Body, BorrowKind, ConstOperand, Local, LocalDecl, MutBorrowKind, Operand, Place, ProjectionElem, Rvalue, SourceInfo, Statement, StatementKind, Terminator, TerminatorKind};
use rustc_middle::mir::patch::MirPatch;
use rustc_middle::mir::CallSource;
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::{source_map::Spanned, DUMMY_SP};

use crate::monitors_finder::MonitorsInfo;
use super::utils::{alloc_unit_local, get_function_generic_args};
use crate::mirpass::FunctionCallInstrumenter;
use crate::utils;


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
