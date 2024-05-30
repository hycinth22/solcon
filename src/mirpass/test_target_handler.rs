// this is a handler for test & templete purpose

use std::collections::HashMap;

use rustc_hir::def_id::DefId;
use rustc_middle::mir::{self, BasicBlock, BasicBlockData, BorrowKind, ConstOperand, Local, LocalDecl, MutBorrowKind, Operand, Place, ProjectionElem, Rvalue, SourceInfo, Statement, StatementKind, Terminator, TerminatorKind};
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::{source_map::Spanned, DUMMY_SP};

use crate::monitors_finder::MonitorsInfo;
use super::utils::{alloc_unit_local, get_function_generic_args};
use crate::mirpass::FunctionCallInstrumenter;

pub struct TestTargetCallHandler{}

impl FunctionCallInstrumenter for TestTargetCallHandler{
    #[inline]
    fn target_function(&self) -> &'static str {
        "this_is_our_test_target_mod::this_is_our_test_target_function"
    }

    fn add_before_handler<'tcx>(&self, tcx: TyCtxt<'tcx>, local_decls: &mut rustc_index::IndexVec<Local, LocalDecl<'tcx>>, 
    this_terminator: &mut Terminator<'tcx>, block: rustc_middle::mir::BasicBlock,
    monitors: &MonitorsInfo,
    ) 
    -> Option<HashMap<BasicBlock, BasicBlockData<'tcx> >>
    {
        let Some(our_func_def_id) = monitors.test_target_before_fn else { warn!("monitors.test_target_before_fn.is_none"); return None; };
        let mut insert_before_call = HashMap::new();
        let call = &mut this_terminator.kind;
        if let TerminatorKind::Call { func, args, destination, target, unwind, call_source, fn_span} = call {
            let generic_args = get_function_generic_args(tcx, local_decls, &func);
            if generic_args.is_none() {
                println!("generic_args.is_none");
                return None;
            }
            let generic_args = generic_args.unwrap();
            // 在函数调用之前插入我们的函数调用需要
            // 1 .更改当前块的terminator call的func到我们的函数，target到我们的新块以便返回后继续在新块执行原调用
            // 2. 把原函数调用移动到下一个我们新生成的基本块，terminator-kind为call，target到当前块的原target
            let ourfunc = {
                let is_generic_func = tcx.generics_of(our_func_def_id).own_requires_monomorphization(); // generics.own_params.is_empty()
                let func_ty = {
                    let binder = tcx.type_of(our_func_def_id);
                    let generics = tcx.generics_of(our_func_def_id);
                    if is_generic_func{
                        binder.instantiate(tcx, generic_args)
                    } else {
                        binder.instantiate_identity()
                    }
                };
                // let instance = tcx.resolve_instance(tcx.param_env(func_def_id).and((func_def_id, generic_args))).unwrap().unwrap();
                // let const_ = mir::Const::zero_sized(instance.instantiate_mir_and_normalize_erasing_regions(
                //     tcx,
                //     ty::ParamEnv::reveal_all(),
                //     ty::EarlyBinder::bind(func_ty),
                // ));
                // Operand::Val(val, func_ty)
                // Operand::Constant(Box::new(ConstOperand {
                //     span: DUMMY_SP,
                //     const_: const_,
                //     user_ty: None,
                // }))
                if is_generic_func {
                    Operand::function_handle(tcx, our_func_def_id, generic_args, fn_span.clone())
                } else {
                    Operand::function_handle(tcx, our_func_def_id, [], fn_span.clone())
                }
            };
            // this_terminator.target will be modify later because new block have not been inserted yet
            let mut our_args = {
                let mut our_args = args.clone();
                // 不能直接clone，因为我们可能会错误地提前move参数，应该由原来的函数调用move它，我们更改所有move为copy（如果参数没有实现copy呢？考虑把所有参数引用化？）
                for arg in our_args.iter_mut() {
                    if let Operand::Move(place) = arg.node {
                        arg.node = Operand::Copy(place);
                    }
                }
                // // 查找所有trait对象的引用
                // for arg in our_args.iter_mut() {
                //     let operand_ty = get_operand_ty(local_decls, &arg.node);
                //     let ty_kind: &rustc_type_ir::TyKind<TyCtxt> = operand_ty.kind();
                //     match ty_kind {
                //         ty::TyKind::Ref() => {
    
                //         }
                //         ty::TyKind::Dynamic() => {
    
                //         }
                //         _ => {}
                //     }
                // }
                our_args
            };
            let bbdata = BasicBlockData {
                statements: vec![],
                terminator: Some(Terminator {
                    kind: TerminatorKind::Call { 
                        func: func.clone(), 
                        args: args.clone(), 
                        destination: destination.clone(), 
                        target: target.clone(),
                        unwind: unwind.clone(), 
                        call_source: call_source.clone(), 
                        fn_span: fn_span.clone() },
                    source_info: this_terminator.source_info.clone(),
                }),
                is_cleanup: false,
            };
            // start to modify
            *func = ourfunc;
            *args = our_args;
            *destination = Place::from(alloc_unit_local(tcx, local_decls));
            for arg in  args.iter_mut() {
                if let Operand::Move(place) = arg.node {
                    arg.node = Operand::Copy(place);
                }
            }
            insert_before_call.insert(block, bbdata);
        }
        Some(insert_before_call)
    }
    
    fn add_after_handler<'tcx>(&self,
        tcx: TyCtxt<'tcx>, local_decls: &mut rustc_index::IndexVec<Local, LocalDecl<'tcx>>, 
        this_terminator: &mut Terminator<'tcx>, block: rustc_middle::mir::BasicBlock,
        monitors: &MonitorsInfo,
    ) -> Option<HashMap<BasicBlock, BasicBlockData<'tcx> >>
    {
        let Some(our_func_def_id) = monitors.test_target_after_fn else { warn!("monitors.test_target_after_fn.is_none"); return None; };
        // 在函数调用之后插入我们的函数调用需要
        // 1 .更改当前块的terminator call的target到我们的新块
        // 2. 在我们新生成的基本块中，terminator-kind为call，func为我们的函数，target到当前块的原target
        // this_terminator.target will be modify later because new block have not been inserted yet
        let mut insert_after_call = HashMap::new();
        let call = &mut this_terminator.kind;
        if let TerminatorKind::Call { func, args, destination, target, unwind, call_source, fn_span} = call {
            let generic_args = get_function_generic_args(tcx, local_decls, &func);
            if generic_args.is_none() {
                println!("generic_args.is_none");
                return None;
            }
            let generic_args = generic_args.unwrap();
            let ourfunc = {
                //dbg!(generic_args);
                let is_generic_func = tcx.generics_of(our_func_def_id).own_requires_monomorphization(); // generics.own_params.is_empty()
                let func_ty = {
                    let binder = tcx.type_of(our_func_def_id);
                    let generics = tcx.generics_of(our_func_def_id);
                    if is_generic_func{
                        binder.instantiate(tcx, generic_args)
                    } else {
                        binder.instantiate_identity()
                    }
                };
                let const_ = mir::Const::zero_sized(func_ty);
                // let instance = tcx.resolve_instance(tcx.param_env(our_func_def_id).and((our_func_def_id, generic_args))).unwrap().unwrap();
                // let const_ = mir::Const::zero_sized(instance.instantiate_mir_and_normalize_erasing_regions(
                //     tcx,
                //     ty::ParamEnv::reveal_all(),
                //     ty::EarlyBinder::bind(func_ty),
                // ));
                //dbg!(func_ty);
                // Operand::Constant(Box::new(ConstOperand {
                //     span: DUMMY_SP,
                //     const_: const_,
                //     user_ty: None,
                // }))
                if is_generic_func {
                    Operand::function_handle(tcx, our_func_def_id, generic_args, fn_span.clone())
                } else {
                    Operand::function_handle(tcx, our_func_def_id, [], fn_span.clone())
                }
            };
    
            // 为了传入返回值，先构造一条创建引用的statement并插到我们的函数调用前
            let ty_dest = local_decls[destination.local].ty;
            let local_decl = LocalDecl::new(Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, ty_dest), fn_span.clone());
            let ref_dest= local_decls.push(local_decl);
    
            let place_destination_ref = Place::from(ref_dest);
            let local_destination_ref_assign_statement: Statement = Statement{
                source_info: SourceInfo::outermost(fn_span.clone()),
                kind: StatementKind::Assign(
                    Box::new((place_destination_ref, Rvalue::Ref(
                        tcx.lifetimes.re_erased,
                        BorrowKind::Mut { kind: MutBorrowKind::Default },
                        destination.clone(),
                    )))
                ),
            };
    
    
            let statements = vec![local_destination_ref_assign_statement];
            let our_args: Vec<Spanned<Operand>> = {
                let mut our_args = args.clone();
    
                // 临时解决方案，阻止原函数调用的操作数move，而交由我们的after函数去处理
                for arg in args {
                    if let Operand::Move(place) = arg.node {
                        arg.node = Operand::Copy(place);
                    }
                }
    
                // 不能直接clone，因为参数可能已被move掉
                // for arg in &mut our_args {
                //     if let Operand::Move(place) = arg.node {
                //         let arg_ty = body.local_decls[place.local].ty;
                //         // 如果原参数是引用被move掉了，我们可以重新创建新的引用。
                //         dbg!(arg_ty.kind());
                //         if let TyKind::Ref(_, ty, mutability) = arg_ty.kind() {
                //             let local_decl = LocalDecl::new(arg_ty, DUMMY_SP);
                //             let new_local= body.local_decls.push(local_decl);
                //             arg.node = Operand::Move(Place::from(new_local));
                            
                //             let local_reref_assign_statement = Statement{
                //                 source_info: SourceInfo::outermost(DUMMY_SP),
                //                 kind: StatementKind::Assign(
                //                     Box::new((place_destination_ref, Rvalue::Ref(
                //                         tcx.lifetimes.re_erased,
                //                         BorrowKind::Shared,
                //                         refed_place, // fuck!!! here need def-use analysis to get it
                //                     )))
                //                 ),
                //             };
                //             statements.push(local_reref_assign_statement);
                //             println!("recreate ref for after handle because moved");
                //         } else {
                //             // 如果原参数是对象被move掉了，我们无法再访问此对象。
                //             println!("after handle cannot access one param because moved"); //（此处可能需要逐个api考虑如何处理）（再想想这里如何处理？）
                //         }
                //     }
                // }
                our_args.push(Spanned{
                    node: Operand::Move(place_destination_ref),
                    span: fn_span.clone(),
                });
                our_args
            };
            let our_dest = Place::from(alloc_unit_local(tcx, local_decls));
            let bbdata = BasicBlockData {
                statements: statements,
                terminator: Some(Terminator {
                    kind: TerminatorKind::Call { 
                        func: ourfunc, 
                        args: our_args, 
                        destination: our_dest, 
                        target: target.clone(),
                        unwind: unwind.clone(), 
                        call_source: call_source.clone(), 
                        fn_span: fn_span.clone() },
                    source_info: this_terminator.source_info.clone(),
                }),
                is_cleanup: false,
            };
            insert_after_call.insert(block, bbdata);
        }
        Some(insert_after_call)
    }
}
