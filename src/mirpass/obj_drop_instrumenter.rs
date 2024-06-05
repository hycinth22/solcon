use crate::monitors_finder::MonitorsInfo;
use rustc_span::def_id::DefId;
use rustc_span::DUMMY_SP;
use rustc_span::source_map::Spanned;
use rustc_span::Span;
use rustc_middle::span_bug;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::TyKind;
use rustc_middle::mir::BasicBlock;
use rustc_middle::mir::BasicBlockData;
use rustc_middle::mir::Body;
use rustc_middle::mir::BorrowKind;
use rustc_middle::mir::CallSource;
use rustc_middle::mir::Const;
use rustc_middle::mir::ConstOperand;
use rustc_middle::mir::ConstValue;
use rustc_middle::mir::Location;
use rustc_middle::mir::Operand;
use rustc_middle::mir::Place;
use rustc_middle::mir::patch::MirPatch;
use rustc_middle::mir::Rvalue;
use rustc_middle::mir::Terminator;
use rustc_middle::mir::TerminatorKind;

use crate::utils;

fn build_drop_callsite_str_operand<'tcx>(
    tcx: TyCtxt<'tcx>, 
    drop_span: Span
) -> Spanned<Operand<'tcx>> {
    let fn_callsite_span_str = utils::span_to_string(tcx, drop_span);
    let alloc = rustc_middle::mir::interpret::Allocation::from_bytes_byte_aligned_immutable(fn_callsite_span_str.as_bytes());
    let const_alloc = tcx.mk_const_alloc(alloc);
    let const_val = ConstValue::Slice{
        data: const_alloc,
        meta: fn_callsite_span_str.len() as u64,
    };
    let const_ty = Ty::new_static_str(tcx);
    Spanned {
        node: Operand::Constant(
            Box::new(ConstOperand{
                span: drop_span.clone(),
                user_ty: None,
                const_: Const::Val(const_val, const_ty),
            }),
        ), 
        span: drop_span.clone(),
    }
}

fn build_drop_span<'tcx>(
    _tcx: TyCtxt<'tcx>, 
    body: &Body<'tcx>, 
    drop_at_block: BasicBlock,
) -> Span {
    let drop_at_block_data = &body.basic_blocks[drop_at_block];
    let drop_location = Location{
        block: drop_at_block,
        statement_index: drop_at_block_data.statements.len(),
    }; // drop must be the terminator of drop_at_block
    let source_info = body.source_info(drop_location);
    source_info.span
}

pub trait ObjectDropInstrumenter {
    fn target_ty(&self) -> &'static str;
    fn before_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId>;
    fn after_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId>;

    fn instrument_drop_before<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, 
        body: &Body<'tcx>, monitors: &MonitorsInfo,
        drop_at_block: BasicBlock,
    ) -> Option<(MirPatch<'tcx>, BasicBlock )> {
        let Some(our_func_def_id) = self.before_monitor_def_id(monitors) else { return None; };
        let drop_at_block_data = &body.basic_blocks[drop_at_block];
        let terminator = drop_at_block_data.terminator();
            match &terminator.kind {
                TerminatorKind::Drop { place, target: _, unwind, replace: _} => {
                    let ty = place.ty(&body.local_decls, tcx).ty;
                    let TyKind::Adt(_adt_def, generic_args) = ty.kind() else {
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
                        build_drop_callsite_str_operand(tcx, build_drop_span(tcx, &body, drop_at_block)),
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
                TerminatorKind::Call{ func, args, destination: _, target: _, unwind, call_source: _, fn_span} => {
                    let Some(generic_args) = utils::get_function_generic_args(tcx, &body.local_decls, &func) else {
                        warn!("Found call to std/core::mem::drop but fail to get function generic_args");
                        unreachable!()
                    };
                    let arg_ty = generic_args.type_at(0);
                    let TyKind::Adt(_adt_def, generic_args) = arg_ty.kind() else {
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
                        Operand::Constant(..) => span_bug!(*fn_span, "running drop on constant")
                    };
                    let temp_ref_to_droping_obj = Place::from(patch.new_temp(Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, arg_ty), *fn_span));
                    patch.add_assign(patch.terminator_loc(body, drop_at_block), temp_ref_to_droping_obj, Rvalue::Ref(
                        tcx.lifetimes.re_erased,
                        BorrowKind::Shared,
                        place_droping_obj,
                    ));
                    let our_call_args = vec![
                        build_drop_callsite_str_operand(tcx, *fn_span),
                        Spanned {
                            node: Operand::Move(temp_ref_to_droping_obj),
                            span: *fn_span,
                        }
                    ];
                    let temp_ret = patch.new_temp(tcx.types.unit, *fn_span);
                    patch.patch_terminator(drop_at_block, TerminatorKind::Call{
                        func: crate::utils::instantiate_our_func(tcx, our_func_def_id, generic_args, *fn_span),
                        args: our_call_args,
                        destination: Place::from(temp_ret),
                        target: Some(new_bb_run_drop),
                        unwind: unwind.clone(),
                        call_source: CallSource::Misc,
                        fn_span: *fn_span,
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
        let Some(our_func_def_id) = self.after_monitor_def_id(monitors) else { return None; };
        let drop_at_block_data = &body.basic_blocks[drop_at_block];
        let terminator = drop_at_block_data.terminator();
        match &terminator.kind {
            TerminatorKind::Drop { place, target, unwind, replace} => {
                let ty = place.ty(&body.local_decls, tcx).ty;
                let TyKind::Adt(_adt_def, generic_args) = ty.kind() else {
                    unreachable!();
                };
                let mut patch = MirPatch::new(body);
                let temp_ref_to_droping_obj = Place::from(patch.new_temp(Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, ty), DUMMY_SP));
                patch.add_assign(patch.terminator_loc(body, drop_at_block), temp_ref_to_droping_obj, Rvalue::Ref(
                    tcx.lifetimes.re_erased,
                    BorrowKind::Shared,
                    place.clone(),
                ));

                let our_call_args = vec![
                    build_drop_callsite_str_operand(tcx, build_drop_span(tcx, &body, drop_at_block)),
                    Spanned {
                        node: Operand::Move(temp_ref_to_droping_obj),
                        span: DUMMY_SP,
                    }
                ];

                let temp_ret = patch.new_temp(tcx.types.unit, DUMMY_SP);
                let new_bb_run_call = patch.new_block(BasicBlockData {
                    statements: vec![],
                    terminator: Some(Terminator {
                        kind: TerminatorKind::Call { 
                            func: crate::utils::instantiate_our_func(tcx, our_func_def_id, generic_args, DUMMY_SP),
                            args: our_call_args, 
                            destination: temp_ret.into(), 
                            target: Some(target.clone()),
                            unwind: unwind.clone(), 
                            call_source: CallSource::Misc, 
                            fn_span: DUMMY_SP },
                        source_info: terminator.source_info.clone(),
                    }),
                    is_cleanup: false,
                });
                patch.patch_terminator(drop_at_block, TerminatorKind::Drop{
                    place: *place,
                    target: new_bb_run_call,
                    unwind: unwind.clone(),
                    replace: replace.clone(),
                });
                return Some((patch, drop_at_block));
            },
            TerminatorKind::Call{ func, args, destination, target, unwind, call_source, fn_span} => {
                let Some(generic_args) = utils::get_function_generic_args(tcx, &body.local_decls, &func) else {
                    warn!("Found call to std/core::mem::drop but fail to get function generic_args");
                    unreachable!()
                };
                let arg_ty = generic_args.type_at(0);
                let TyKind::Adt(_adt_def, generic_args) = arg_ty.kind() else {
                    unreachable!()
                };
                let mut patch = MirPatch::new(body);
                let place_droping_obj = match args[0].node {
                    Operand::Copy(place) | Operand::Move(place) => place,
                    Operand::Constant(..) => span_bug!(*fn_span, "running drop on constant")
                };
                let temp_ref_to_droping_obj = Place::from(patch.new_temp(Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, arg_ty), *fn_span));
                patch.add_assign(patch.terminator_loc(body, drop_at_block), temp_ref_to_droping_obj, Rvalue::Ref(
                    tcx.lifetimes.re_erased,
                    BorrowKind::Shared,
                    place_droping_obj,
                ));
                let our_call_args = vec![
                    build_drop_callsite_str_operand(tcx, *fn_span),
                    Spanned {
                        node: Operand::Move(temp_ref_to_droping_obj),
                        span: *fn_span,
                    }
                ];
                let temp_ret = patch.new_temp(tcx.types.unit, *fn_span);
                let new_bb_run_call = patch.new_block(BasicBlockData {
                    statements: vec![],
                    terminator: Some(Terminator {
                        kind: TerminatorKind::Call { 
                            func: crate::utils::instantiate_our_func(tcx, our_func_def_id, generic_args, *fn_span),
                            args: our_call_args, 
                            destination: temp_ret.into(), 
                            target: target.clone(),
                            unwind: unwind.clone(), 
                            call_source: call_source.clone(), 
                            fn_span: fn_span.clone() },
                        source_info: terminator.source_info.clone(),
                    }),
                    is_cleanup: false,
                });
                patch.patch_terminator(drop_at_block, TerminatorKind::Call{
                    func: func.clone(),
                    args: args.clone(),
                    destination: destination.clone(),
                    target: Some(new_bb_run_call),
                    unwind: unwind.clone(),
                    call_source: call_source.clone(),
                    fn_span: fn_span.clone(),
                });
                return Some((patch, drop_at_block));
            },
            _  => unreachable!(),
        }
    }
}