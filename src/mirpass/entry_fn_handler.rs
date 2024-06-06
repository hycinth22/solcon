use std::vec;

use crate::monitors_finder::MonitorsInfo;
use rustc_span::def_id::DefId;
use rustc_span::source_map::Spanned;
use rustc_span::Span;
use rustc_middle::bug;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::mir::BasicBlock;
use rustc_middle::mir::BasicBlockData;
use rustc_middle::mir::Body;
use rustc_middle::mir::BorrowKind;
use rustc_middle::mir::CallSource;
use rustc_middle::mir::Const;
use rustc_middle::mir::ConstOperand;
use rustc_middle::mir::ConstValue;
use rustc_middle::mir::Operand;
use rustc_middle::mir::Place;
use rustc_middle::mir::patch::MirPatch;
use rustc_middle::mir::Rvalue;
use rustc_middle::mir::Statement;
use rustc_middle::mir::StatementKind;
use rustc_middle::mir::SourceInfo;
use rustc_middle::mir::Terminator;
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::TyKind;
use rustc_middle::mir::MutBorrowKind;
use rustc_middle::mir::UnwindAction;
use rustc_middle::mir::UnwindTerminateReason;

use crate::utils;

#[derive(Default)]
pub struct EntryFnBodyInstrumenter();

impl EntryFnBodyInstrumenter {
    pub fn instrument_body<'tcx>(&self,
        tcx: TyCtxt<'tcx>, 
        body: &mut Body<'tcx>, monitors: &MonitorsInfo,
    ) {
        self.instrument_body_before(tcx, body, monitors);
        self.instrument_body_after(tcx, body, monitors);
    }

    fn instrument_body_before<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, 
        body: &mut Body<'tcx>, monitors: &MonitorsInfo,
    ) {
        let Some(our_func_def_id) = monitors.entry_fn_before_fn else {
            return;
        };
        let Some((first_bb, first_bb_data)) = body.basic_blocks.iter_enumerated().next() else {
            return;
        };
        let body_span = body.span;
        let mut patch = MirPatch::new(body);
        let new_bb_run_origin_first = patch.new_block(first_bb_data.clone());
        let temp_ret = patch.new_temp(tcx.types.unit, body_span.clone());
        patch.patch_terminator(first_bb, TerminatorKind::Call{
            func: Operand::function_handle(tcx, our_func_def_id, [], body_span.clone()),
            args: vec![],
            destination: Place::from(temp_ret),
            target: Some(new_bb_run_origin_first),
            unwind: UnwindAction::Continue,
            call_source: CallSource::Misc,
            fn_span: body_span.clone(),
        });
        patch.apply(body);
        let Some(first_bb_data) = body.basic_blocks.as_mut().iter_mut().next() else {
            unreachable!()
        };
        first_bb_data.statements.clear();
    }

    fn instrument_body_after<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, 
        body: &mut Body<'tcx>, monitors: &MonitorsInfo,
    ) {
        let Some(our_func_def_id) = monitors.entry_fn_after_fn else {
            return ;
        };
        let body_span = body.span;
        let mut patch = MirPatch::new(body);
        for (bb, bb_data) in body.basic_blocks.iter_enumerated() {
            let terminator = bb_data.terminator();
            match &terminator.kind {
                TerminatorKind::Return => {
                    // 捕获函数正常返回。
                    let new_bb_run_original_return = patch.new_block(BasicBlockData {
                        statements: vec![],
                        terminator: Some(terminator.clone()),
                        is_cleanup: bb_data.is_cleanup,
                    });
                    let temp_ret = patch.new_temp(tcx.types.unit, body_span.clone());
                    patch.patch_terminator(bb, TerminatorKind::Call{
                        func: Operand::function_handle(tcx, our_func_def_id, [], body_span.clone()),
                        args: vec![],
                        destination: Place::from(temp_ret),
                        target: Some(new_bb_run_original_return),
                        unwind: UnwindAction::Continue,
                        call_source: CallSource::Misc,
                        fn_span: body_span.clone(),
                    });
                }
                TerminatorKind::UnwindResume | TerminatorKind::UnwindTerminate(..) => {
                    // 当前不捕获unwind引发的函数调用结束
                    // 即使我们可以捕获入口函数中的所有UnwindResume和UnwindTerminate
                    // 但无法捕获子函数调用中的UnwindTerminate（通常是因为处理unwind进行cleanup时再次unwind）
                    // (一种可能的实现：可以查找所有Termiantor中unwind: UnwindAction==Terminate，更换为UnwindAction::Cleanup(bb)再在bb中调用monitor和TermiantorKind::UnwindTerminate(orignalReason)，但这需要修改所有函数，且我们也无法覆盖到所有函数)
                    // 可以在monitor中使用std::panic::set_hook捕获整个程序的panic
                }
                TerminatorKind::Goto{..} | TerminatorKind::SwitchInt{..} | TerminatorKind::Unreachable => {}, // safe and donot cause the end of this invocation of the function.
                TerminatorKind::Drop{..} | TerminatorKind::Call{..} | TerminatorKind::Assert{..} => {}, // only end this basic blocks, we process unwind if any panics
                TerminatorKind::Yield{..} | TerminatorKind::CoroutineDrop => bug!("entry fn should not be a coroutine"),
                TerminatorKind::FalseEdge{..} | TerminatorKind::FalseUnwind{..} => bug!("TerminatorKind::FalseEdge | TerminatorKind::FalseUnwind is disallowed after drop elaboration"),
                TerminatorKind::InlineAsm{..} => {}
            }
        }
        patch.apply(body);
    }
}