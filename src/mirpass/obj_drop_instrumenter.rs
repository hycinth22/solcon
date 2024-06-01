use crate::monitors_finder::MonitorsInfo;
use rustc_span::def_id::DefId;
use rustc_span::DUMMY_SP;
use rustc_span::source_map::Spanned;
use rustc_middle::ty::GenericArgs;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_middle::mir::BasicBlock;
use rustc_middle::mir::BasicBlockData;
use rustc_middle::mir::Body;
use rustc_middle::mir::BorrowKind;
use rustc_middle::mir::CallSource;
use rustc_middle::mir::Operand;
use rustc_middle::mir::Place;
use rustc_middle::mir::patch::MirPatch;
use rustc_middle::mir::Rvalue;
use rustc_middle::mir::Statement;
use rustc_middle::mir::StatementKind;
use rustc_middle::mir::SourceInfo;
use rustc_middle::mir::Terminator;
use rustc_middle::mir::TerminatorKind;
use rustc_middle::mir::MutBorrowKind;

use crate::utils;

pub trait ObjectDropInstrumenter {
    fn target_ty(&self) -> &'static str;
    fn before_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId>;
    fn after_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId>;

    fn instrument_drop_before<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, 
        body: &Body<'tcx>, monitors: &MonitorsInfo,
        drop_at_block: BasicBlock,
    ) -> Option<(MirPatch<'tcx>, BasicBlock )> {
        let Some(our_func_def_id) = self.before_monitor_def_id(monitors) else { warn!("monitors.mutexguard_drop_before_fn.is_none"); return None; };
        let drop_at_block_data = &body.basic_blocks[drop_at_block];
        let terminator = drop_at_block_data.terminator();
            match &terminator.kind {
                TerminatorKind::Drop { place, target, unwind, replace} => {
                    let ty = place.ty(&body.local_decls, tcx).ty;
                    let TyKind::Adt(adt_def, generic_args) = ty.kind() else {
                        unreachable!();
                    };
                    let mut patch = MirPatch::new(body);
                    let new_bb_run_drop = patch.new_block(BasicBlockData{
                        statements: vec![],
                        terminator: Some(terminator.clone()),
                        is_cleanup: drop_at_block_data.is_cleanup,
                    });
                    let temp_ref_to_droping_obj = Place::from(patch.new_temp(Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, ty), DUMMY_SP));
                    patch.add_assign(patch.terminator_loc(body, drop_at_block), temp_ref_to_droping_obj, Rvalue::Ref(
                        tcx.lifetimes.re_erased,
                        BorrowKind::Shared,
                        place.clone(),
                    ));
                    let our_call_args = vec![
                        Spanned {
                            node: Operand::Move(temp_ref_to_droping_obj),
                            span: DUMMY_SP,
                        }
                    ];
                    let temp_ret = patch.new_temp(tcx.types.unit, DUMMY_SP);
                    patch.patch_terminator(drop_at_block, TerminatorKind::Call{
                        func: crate::utils::instantiate_our_func(tcx, our_func_def_id, generic_args, DUMMY_SP),
                        args: our_call_args,
                        destination: Place::from(temp_ret),
                        target: Some(new_bb_run_drop),
                        unwind: unwind.clone(),
                        call_source: CallSource::Misc,
                        fn_span: DUMMY_SP,
                    });
                    Some((patch, new_bb_run_drop))
                },
                TerminatorKind::Call{ func, args, destination, target, unwind, call_source, fn_span} => {
                    let Some(generic_args) = utils::get_function_generic_args(tcx, &body.local_decls, &func) else {
                        warn!("Found call to std/core::mem::drop but fail to get function generic_args");
                        unreachable!()
                    };
                    let arg_ty = generic_args.type_at(0);
                    let TyKind::Adt(adt_def, generic_args) = arg_ty.kind() else {
                        unreachable!()
                    };
                    let mut patch = MirPatch::new(body);
                    let new_bb_run_drop = patch.new_block(BasicBlockData{
                        statements: vec![],
                        terminator: Some(terminator.clone()),
                        is_cleanup: drop_at_block_data.is_cleanup,
                    });
                    let place_droping_obj = match args[0].node {
                        Operand::Copy(place) | Operand::Move(place) => place,
                        Operand::Constant(..) => panic!("running drop on constant")
                    };
                    let temp_ref_to_droping_obj = Place::from(patch.new_temp(Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, arg_ty), DUMMY_SP));
                    patch.add_assign(patch.terminator_loc(body, drop_at_block), temp_ref_to_droping_obj, Rvalue::Ref(
                        tcx.lifetimes.re_erased,
                        BorrowKind::Shared,
                        place_droping_obj,
                    ));
                    let our_call_args = vec![
                        Spanned {
                            node: Operand::Move(temp_ref_to_droping_obj),
                            span: DUMMY_SP,
                        }
                    ];
                    let temp_ret = patch.new_temp(tcx.types.unit, DUMMY_SP);
                    patch.patch_terminator(drop_at_block, TerminatorKind::Call{
                        func: crate::utils::instantiate_our_func(tcx, our_func_def_id, generic_args, DUMMY_SP),
                        args: our_call_args,
                        destination: Place::from(temp_ret),
                        target: Some(new_bb_run_drop),
                        unwind: unwind.clone(),
                        call_source: CallSource::Misc,
                        fn_span: DUMMY_SP,
                    });
                    Some((patch, new_bb_run_drop))
                }
                _  => unreachable!(),
            }
    }

    fn instrument_drop_after<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, 
        body: &Body<'tcx>, monitors: &MonitorsInfo,
        drop_at_block: BasicBlock
    ) -> Option<(MirPatch<'tcx>, BasicBlock )> {
        None
    }
}