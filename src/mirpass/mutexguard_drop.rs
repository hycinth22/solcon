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
use crate::monitors_finder::MonitorsInfo;
use std::collections::HashMap;
use crate::utils;

pub struct MutexGuardDropPass{}
const TARGET_TYPE : &'static str = "std::sync::MutexGuard";

impl crate::mirpass::OurMirPass for MutexGuardDropPass {
    fn run_pass<'tcx>(&self, 
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>, monitors: &MonitorsInfo)
    -> Option< MirPatch<'tcx> > 
    {
        let mut patch: Option< MirPatch<'tcx> >  = None;
        let Some(our_func_def_id) = monitors.mutexguard_drop_before_fn else { warn!("monitors.mutexguard_drop_before_fn.is_none"); return None; };
        for (block, block_data) in body.basic_blocks.iter_enumerated() {
            let terminator = block_data.terminator();
            match &terminator.kind {
                TerminatorKind::Drop { place, target, unwind, replace} => {
                    let ty = body.local_decls[place.local].ty;
                    if let TyKind::Adt(adt_def, generic_args) = ty.kind() {
                        let ty_def_id = adt_def.did();
                        let ty_def_path_str = tcx.def_path_str(ty_def_id);
                        info!("found drop of {}", ty_def_path_str);
                        if ty_def_path_str== TARGET_TYPE {
                            if patch.is_none() {
                                patch = Some(MirPatch::new(body));
                            }
                            let mut patch = patch.as_mut().unwrap();
                            let new_bb_run_drop = patch.new_block(BasicBlockData{
                                statements: vec![],
                                terminator: Some(terminator.clone()),
                                is_cleanup: block_data.is_cleanup,
                            });
                            let temp_ref_to_droping_obj = Place::from(patch.new_temp(Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, ty), DUMMY_SP));
                            patch.add_assign(patch.terminator_loc(body, block), temp_ref_to_droping_obj, Rvalue::Ref(
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
                            patch.patch_terminator(block, TerminatorKind::Call{
                                func: crate::utils::instantiate_our_func(tcx, our_func_def_id, generic_args, DUMMY_SP),
                                args: our_call_args,
                                destination: Place::from(temp_ret),
                                target: Some(new_bb_run_drop),
                                unwind: unwind.clone(),
                                call_source: CallSource::Misc,
                                fn_span: DUMMY_SP,
                            });
                        }
                    }
                }
                TerminatorKind::Call{ func, args, destination, target, unwind, call_source, fn_span} => {
                    let Some(func_def_path_str) = utils::get_function_path_str(tcx, &body.local_decls, &func) else {
                        warn!("Found call to function but fail to get function DefPath");
                        continue;
                    };
                    if func_def_path_str == "std::mem::drop" || func_def_path_str == "core::mem::drop" {
                        let Some(generic_args) = utils::get_function_generic_args(tcx, &body.local_decls, &func) else {
                            warn!("Found call to mem::drop but fail to get function generic_args");
                            continue;
                        };
                        let arg_ty = generic_args.type_at(0);
                        if let TyKind::Adt(adt_def, generic_args) = arg_ty.kind() {
                            let ty_def_id = adt_def.did();
                            let ty_def_path_str = tcx.def_path_str(ty_def_id);
                            info!("found call to drop of {func_def_path_str} for type {ty_def_path_str}");
                            if ty_def_path_str == TARGET_TYPE {
                                if patch.is_none() {
                                    patch = Some(MirPatch::new(body));
                                }
                                let mut patch = patch.as_mut().unwrap();
                                let new_bb_run_drop = patch.new_block(BasicBlockData{
                                    statements: vec![],
                                    terminator: Some(terminator.clone()),
                                    is_cleanup: block_data.is_cleanup,
                                });
                                let place_droping_obj = match args[0].node {
                                    Operand::Copy(place) | Operand::Move(place) => place,
                                    Operand::Constant(..) => panic!("running drop on constant")
                                };
                                let temp_ref_to_droping_obj = Place::from(patch.new_temp(Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, arg_ty), DUMMY_SP));
                                patch.add_assign(patch.terminator_loc(body, block), temp_ref_to_droping_obj, Rvalue::Ref(
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
                                patch.patch_terminator(block, TerminatorKind::Call{
                                    func: crate::utils::instantiate_our_func(tcx, our_func_def_id, generic_args, DUMMY_SP),
                                    args: our_call_args,
                                    destination: Place::from(temp_ret),
                                    target: Some(new_bb_run_drop),
                                    unwind: unwind.clone(),
                                    call_source: CallSource::Misc,
                                    fn_span: DUMMY_SP,
                                });
                            }
                        } else {
                            info!("found call to drop of {func_def_path_str} but type is not adt");
                        }
                    }
                },
                _  => {},
            }
        }
        
        patch
    }
}