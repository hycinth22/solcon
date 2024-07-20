use std::collections::HashMap;
use rustc_middle::mir::Body;
use rustc_middle::mir::{self, BasicBlock, BasicBlockData, BorrowKind, ConstOperand, Local, LocalDecl, MutBorrowKind, Operand, Place, ProjectionElem, Rvalue, SourceInfo, Statement, StatementKind, Terminator, TerminatorKind};
use rustc_middle::mir::patch::MirPatch;
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::{source_map::Spanned, DUMMY_SP};
use rustc_span::def_id::DefId;
use crate::{mirpass::FunctionCallInstrumenter, monitors_finder::MonitorsInfo, utils::{alloc_unit_local, get_function_generic_args}};
use crate::utils;

#[derive(Default)]
pub struct MutexLockResultUnwrapCallHandler<'pass>{
    __marker: std::marker::PhantomData<&'pass str>,
}

impl<'pass> FunctionCallInstrumenter<'_> for MutexLockResultUnwrapCallHandler<'_> {
    #[inline]
    fn target_function(&self) -> &'static str {
        "std::result::Result"
    }

    #[inline]
    fn before_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> {
        let Some(our_func_def_id) = monitors.mutex_lock_before_fn else { warn!("monitors.mutex_lock_before_fn.is_none"); return None; };
        Some(our_func_def_id)
    }

    #[inline]
    fn after_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> {
        let Some(our_func_def_id) = monitors.mutex_lock_after_fn else { warn!("monitors.mutex_lock_after_fn.is_none"); return None; };
        Some(our_func_def_id)
    }
}

