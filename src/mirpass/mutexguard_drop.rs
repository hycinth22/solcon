use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_middle::mir::BasicBlockData;
use rustc_middle::mir::Body;
use rustc_middle::mir::BorrowKind;
use rustc_middle::mir::CallSource;
use rustc_middle::mir::Operand;
use rustc_middle::mir::Place;
use rustc_middle::mir::patch::MirPatch;
use rustc_middle::mir::Rvalue;
use rustc_middle::mir::TerminatorKind;
use rustc_span::{source_map::Spanned, DUMMY_SP};
use rustc_span::def_id::DefId;
use crate::monitors_finder::MonitorsInfo;
use std::collections::HashMap;
use crate::utils;

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