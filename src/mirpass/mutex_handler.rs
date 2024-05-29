use std::collections::HashMap;

use rustc_middle::mir::{self, BasicBlock, BasicBlockData, BorrowKind, ConstOperand, Local, LocalDecl, MutBorrowKind, Operand, Place, ProjectionElem, Rvalue, SourceInfo, Statement, StatementKind, Terminator, TerminatorKind};
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::{source_map::Spanned, DUMMY_SP};
use rustc_span::def_id::DefId;
use crate::{monitors_finder::MonitorsInfo, utils::{alloc_unit_local, get_function_generic_args}};

pub(crate) fn add_mutex_lock_before_handler<'tcx>(tcx: TyCtxt<'tcx>, local_decls: &mut rustc_index::IndexVec<Local, LocalDecl<'tcx>>, 
this_terminator: &mut Terminator<'tcx>, block: rustc_middle::mir::BasicBlock,
our_func_def_id: DefId
) 
-> HashMap<BasicBlock, BasicBlockData<'tcx> >
{
    let mut insert_before_call = HashMap::new();
    let call = &mut this_terminator.kind;
    if let TerminatorKind::Call { func, args, destination, target, unwind, call_source, fn_span} = call {
        let generic_args = get_function_generic_args(local_decls, &func);
        if generic_args.is_none() {
            println!("generic_args.is_none");
            return insert_before_call;
        }
        let mut generic_args = generic_args.unwrap();
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
            if is_generic_func {
                Operand::function_handle(tcx, our_func_def_id, generic_args, fn_span.clone())
            } else {
                Operand::function_handle(tcx, our_func_def_id, [], fn_span.clone())
            }
        };
        // this_terminator.target will be modify later because new block have not been inserted yet
        let mut our_args = {
            // 不能直接clone，因为我们可能会错误地提前move参数，应该由原来的函数调用move它，我们更改所有move为copy（如果参数没有实现copy呢？考虑把所有参数引用化？）
            let mut our_args = args.clone();
            for arg in our_args.iter_mut() {
                if let Operand::Move(place) = arg.node {
                    arg.node = Operand::Copy(place);
                }
            }
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
    insert_before_call
}

pub(crate) fn add_mutex_lock_after_handler<'tcx>(
    tcx: TyCtxt<'tcx>, local_decls: &mut rustc_index::IndexVec<Local, LocalDecl<'tcx>>, 
    this_terminator: &mut Terminator<'tcx>, block: rustc_middle::mir::BasicBlock,
    our_func_def_id: DefId
) -> HashMap<BasicBlock, BasicBlockData<'tcx> >
{
    // 在函数调用之后插入我们的函数调用需要
    // 1 .更改当前块的terminator call的target到我们的新块
    // 2. 在我们新生成的基本块中，terminator-kind为call，func为我们的函数，target到当前块的原target
    // this_terminator.target will be modify later because new block have not been inserted yet
    let mut insert_after_call = HashMap::new();
    let call = &mut this_terminator.kind;
    if let TerminatorKind::Call { func, args, destination, target, unwind, call_source, fn_span} = call {
        let generic_args = get_function_generic_args(local_decls, &func);
        if generic_args.is_none() {
            println!("generic_args.is_none");
            return insert_after_call;
        }
        let generic_args = generic_args.unwrap();
        let ourfunc = {
            // dbg!(generic_args);
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
            if is_generic_func {
                Operand::function_handle(tcx, our_func_def_id, generic_args, fn_span.clone())
            } else {
                Operand::function_handle(tcx, our_func_def_id, [], fn_span.clone())
            }
        };

        // 为了传入返回值，先构造一条创建引用的statement并插到我们的函数调用前
        let ty_dest = local_decls[destination.local].ty;
        let local_decl = LocalDecl::new(Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, ty_dest), DUMMY_SP);
        let ref_dest= local_decls.push(local_decl);

        let place_destination_ref = Place::from(ref_dest);
        let local_destination_ref_assign_statement = Statement{
            source_info: SourceInfo::outermost(DUMMY_SP),
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

            our_args.push(Spanned{
                node: Operand::Move(place_destination_ref),
                span: DUMMY_SP,
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
    insert_after_call
}
