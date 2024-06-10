use rustc_middle::mir::{BasicBlock, Operand, TerminatorKind};
use rustc_middle::mir::Body;
use rustc_middle::mir::patch::MirPatch;
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::DefId;

use crate::{mirpass::FunctionCallInstrumenter, monitors_finder::MonitorsInfo};

// For debugging only
#[derive(Default)]
pub struct FunctionCallInspectorInstrumenter<'pass>{
    target_function: &'pass str,
}

impl<'pass> FunctionCallInstrumenter<'_> for FunctionCallInspectorInstrumenter<'_>{
    #[inline]
    fn target_function(&self) -> &'static str {
        "update_twice"
    }
    #[inline]
    fn before_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> { 
        unreachable!()
    }
    #[inline]
    fn after_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId> { 
        unreachable!()
    }

    fn instrument_call_before<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, 
        body: &Body<'tcx>, monitors: &MonitorsInfo,
        call_at_block: BasicBlock, 
    ) -> Option<(MirPatch<'tcx>, BasicBlock )>
    {
        let terminator = &body.basic_blocks[call_at_block].terminator();
        if let TerminatorKind::Call { func, args, destination, target, unwind, call_source, fn_span} = &terminator.kind {
            for arg in args.iter() {
                match &arg.node {
                    Operand::Move(place) => {
                        info!("move");
                        dbg!(place);
                    },
                    Operand::Copy(place) => {
                        info!("copy");
                        dbg!(place);
                    },
                    Operand::Constant(box const_operand) => {
                        info!("Constant");
                        dbg!(const_operand);
                    },
                }
            }
        }
        None
    }
    
    fn instrument_call_after<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, 
        body: &Body<'tcx>, monitors: &MonitorsInfo,
        call_at_block: BasicBlock, 
    ) -> Option<(MirPatch<'tcx>, BasicBlock )>
    {
        None
    }    
}

