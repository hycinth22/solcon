use crate::monitors_finder::MonitorsInfo;
use rustc_span::def_id::DefId;
use rustc_span::source_map::Spanned;
use rustc_span::Span;
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

use crate::utils;

pub(crate) fn build_monitor_args<'tcx>(patch: &mut MirPatch<'tcx>, 
    original_args: &Vec<Spanned<Operand<'tcx>>>, no_instantiate_func_args_tys: Vec<&Ty>,
    tcx: TyCtxt<'tcx>, 
    body: &Body<'tcx>, assign_ref_block: BasicBlock, 
    fn_span: &Span,
 )  -> Vec<Spanned<Operand<'tcx>>> {
    let transfromed_args = original_args.iter().zip(no_instantiate_func_args_tys.iter()).map(|(arg, call_arg_ty)| {
        let arg_ty = arg.node.ty(&body.local_decls, tcx);
        // 注意：
        // 1. 不能直接clone Operand::Move，因为如果是move语义传递的参数，则我们会错误地提前move参数（正确行为：应该由原来的函数调用move它）
        // 2. 也不能简单地将Operand::Move更改为Operand::Copy，因为这会导致在需要drop的类型的同一对象上运行两次deconstructor。
        // 所以，我们在这里我们仅保持指针类型(reference, raw pointer, fn pointer) 按Copy传递，
        // 将所有其他参数更改为reference传递。
        let operand = if call_arg_ty.is_any_ptr() {
            debug!("call_arg_ty.is_any_ptr()");
            match arg.node {
                // 对于reference类型，虽然仅&T实现了Copy trait而&mut T没有Copy trait（https://doc.rust-lang.org/stable/src/core/marker.rs.html#437），
                // 但是我们在这里仍可以安全地复制&mut T。理由是，Copy trait的意义在于保证可以按位复制不需要deconstructor（drop），而我们的pass运行在optimized_mir（MIR to Binaries阶段）已经运行过analysis阶段，所以可以对任意类型（无论是否实现Copy trait）进行bitwise Copy而不会影响drop elaboration
                Operand::Move(place) => Operand::Copy(place),
                Operand::Copy(..) | Operand::Constant(..) => arg.node.clone(),
            }
        } else {
            debug!("!call_arg_ty.is_any_ptr()");
            // 理论上只要没有Drop trait（特别是实现了Copy trait），我们仍然可以按照Copy直接传递对象，但是这样 1. 可能有效率问题 2. 带来接口的不统一（需要人工查看每个类型是否有Drop trait）
            // 所以，在这里，我们简单地统一对于将所有非指针类型(非reference, raw pointer, fn pointer)参数转换为reference传递
            match arg.node {
                Operand::Move(place) | Operand::Copy(place) => {
                    let local_temp_ref_to_this_arg = patch.new_temp(Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, arg_ty), fn_span.clone());
                    patch.add_assign(patch.terminator_loc(body, assign_ref_block), local_temp_ref_to_this_arg.into(), Rvalue::Ref(
                        tcx.lifetimes.re_erased,
                        BorrowKind::Shared,
                        place.clone(),
                    ));
                    Operand::Move(local_temp_ref_to_this_arg.into())
                }
                Operand::Constant(..) => {
                    panic!("how to create ref to constat?")
                }
            }
        };
        Spanned {
            node: operand, 
            span: arg.span.clone(),
        }
    });
    let mut result = Vec::new();
    result.push({
        // build fn_callsite_span_str as first arg
        let fn_callsite_span_str = utils::span_to_string(tcx, *fn_span);
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
                    span: fn_span.clone(),
                    user_ty: None,
                    const_: Const::Val(const_val, const_ty),
                }),
            ), 
            span: fn_span.clone(),
        }
    });
    result.extend(transfromed_args);
    result
}


 // tyobj.fn_sig() 可以获得函数签名。但倘若tyobj包含了泛型参数，会获得具体化的函数参数类型列表
 // 本函数可以获得保持泛型的函数参数类型列表
pub(crate) fn get_no_instantiate_func_args_tys_from_fn_ty<'tcx>(tcx: TyCtxt<'tcx>, func_ty_with_generic_args: &Ty) -> Option<Vec<&'tcx Ty<'tcx>>> {
    let func_def_id = {
        let kind = func_ty_with_generic_args.kind();
        match kind {
            TyKind::FnDef(def_id, ..) => def_id, // The anonymous type of a function declaration/definition
            TyKind::Closure(def_id, _args) => def_id, // // The anonymous type of a closure. Used to represent the type of |a| a.
            TyKind::CoroutineClosure(def_id, _args) => def_id,  // The anonymous type of a closure. Used to represent the type of async |a| a.
            TyKind::Coroutine(def_id, _args) => def_id, // The anonymous type of a coroutine. Used to represent the type of |a| yield a.
            TyKind::FnPtr(_) => {
                warn!("get_no_instantiate_func_args_tys failed: we cannot infer FnPtr point to what");
                return None;
            },
            _ => unreachable!(),
        }
    };
    let func_sig = tcx.fn_sig(func_def_id).skip_binder();
    let func_sig_arg_tys : Vec<_> = func_sig.inputs().iter().map(|binder| binder.skip_binder() ).collect();
    Some(func_sig_arg_tys)
}

pub trait FunctionCallInstrumenter<'pass> {
    fn target_function(&self) -> &'pass str;
    fn before_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId>;
    fn after_monitor_def_id(&self, monitors: &MonitorsInfo) -> Option<DefId>;

    fn instrument_call_before<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, 
        body: &Body<'tcx>, monitors: &MonitorsInfo,
        call_at_block: BasicBlock, 
    ) -> Option<(MirPatch<'tcx>, BasicBlock )> {
        let Some(our_func_def_id) = self.before_monitor_def_id(monitors) else {
            return None;
        };
        let terminator = &body.basic_blocks[call_at_block].terminator();
        let call = &terminator.kind;
        if let TerminatorKind::Call { func, args, destination, target, unwind, call_source, fn_span} = call {
            let func_ty_with_generic_args = func.ty(&body.local_decls, tcx);
            let Some(no_instantiate_func_args_tys) = get_no_instantiate_func_args_tys_from_fn_ty(tcx, &func_ty_with_generic_args) else {
                return None;
            };
            let generic_args = utils::get_function_generic_args(tcx, &body.local_decls, &func);
            if generic_args.is_none() {
                warn!("target_function {} generic_args.is_none", self.target_function());
                return None;
            }
            let generic_args = generic_args.unwrap();
            let mut patch = MirPatch::new(body);
            // 在函数调用之前插入我们的函数调用需要
            // 1. 把原函数调用移动到下一个我们新生成的基本块，terminator-kind为call，target到当前块的原target
            // 2 .更改当前块的terminator call的func到我们的函数，target到我们的新块以便我们的函数返回后继续在新块执行原调用
            let our_call_args = build_monitor_args(&mut patch, args, no_instantiate_func_args_tys, tcx, body, call_at_block, fn_span);
            let new_bb_run_call = patch.new_block(BasicBlockData {
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
                    source_info: terminator.source_info.clone(),
                }),
                is_cleanup: false,
            });
            let temp_ret = patch.new_temp(tcx.types.unit, fn_span.clone());
            patch.patch_terminator(call_at_block, TerminatorKind::Call{
                func: utils::instantiate_our_func(tcx, our_func_def_id, generic_args, fn_span.clone()),
                args: our_call_args,
                destination: Place::from(temp_ret),
                target: Some(new_bb_run_call),
                unwind: unwind.clone(),
                call_source: CallSource::Misc,
                fn_span: fn_span.clone(),
            });
            return Some((patch, new_bb_run_call));
        } else {
            unreachable!("parameter always be TerminatorKind::Call")
        }
    }
    fn instrument_call_after<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, 
        body: &Body<'tcx>, monitors: &MonitorsInfo,
        call_at_block: BasicBlock, 
    ) -> Option<(MirPatch<'tcx>, BasicBlock)> {
        let Some(our_func_def_id) = self.after_monitor_def_id(monitors) else {
            return None;
        };
        let terminator = &body.basic_blocks[call_at_block].terminator();
        if let TerminatorKind::Call { func, args, destination, target, unwind, call_source, fn_span} = &terminator.kind {
            let func_ty_with_generic_args = func.ty(&body.local_decls, tcx);
            let Some(no_instantiate_func_args_tys) = get_no_instantiate_func_args_tys_from_fn_ty(tcx, &func_ty_with_generic_args) else {
                return None;
            };
            let generic_args = utils::get_function_generic_args(tcx, &body.local_decls, &func);
            if generic_args.is_none() {
                warn!("target_function {} generic_args.is_none", self.target_function());
                return None;
            }
            let generic_args = generic_args.unwrap();
            let mut patch = MirPatch::new(body);
            // 在函数调用之后插入我们的函数调用需要
            // 1 .更改当前块的terminator call的target到我们的新块
            // 2. 在我们新生成的基本块中，terminator-kind为call，func为我们的函数，target到当前块的原target

            // 为了传入返回值，先构造一条创建引用的statement并插到我们的函数调用前
            let ty_dest = destination.ty(&body.local_decls, tcx).ty;
            let local_tmp_ref_to_dest = patch.new_temp(Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, ty_dest), fn_span.clone());
            let statements = vec![Statement{
                source_info: SourceInfo::outermost(fn_span.clone()),
                kind: StatementKind::Assign(
                    Box::new((Place::from(local_tmp_ref_to_dest), Rvalue::Ref(
                        tcx.lifetimes.re_erased,
                        BorrowKind::Mut { kind: MutBorrowKind::Default },
                        destination.clone(),
                    )))
                ),
            }];
            // Notice: here may cause problems because a reference to obj may be passed after moving and dropping it. This behavior may change in the future.
            let mut our_call_args = build_monitor_args(&mut patch, args, no_instantiate_func_args_tys, tcx, body, call_at_block, fn_span);
            our_call_args.push(Spanned{
                node: Operand::Move(Place::from(local_tmp_ref_to_dest)),
                span: fn_span.clone(),
            });
            let temp_our_dest = patch.new_temp(tcx.types.unit, fn_span.clone());
            let new_bb_run_our_func_call = patch.new_block(BasicBlockData {
                statements: statements,
                terminator: Some(Terminator {
                    kind: TerminatorKind::Call { 
                        func: utils::instantiate_our_func(tcx, our_func_def_id, generic_args, fn_span.clone()), 
                        args: our_call_args, 
                        destination: Place::from(temp_our_dest), 
                        target: target.clone(),
                        unwind: unwind.clone(), 
                        call_source: call_source.clone(), 
                        fn_span: fn_span.clone() },
                    source_info: terminator.source_info.clone(),
                }),
                is_cleanup: false,
            });
            patch.patch_terminator(call_at_block, TerminatorKind::Call{
                func: func.clone(),
                args: args.clone(),
                destination: destination.clone(),
                target: Some(new_bb_run_our_func_call),
                unwind: unwind.clone(),
                call_source: call_source.clone(),
                fn_span: fn_span.clone(),
            });
            return Some((patch, call_at_block));
        } else {
            unreachable!("parameter always be TerminatorKind::Call")
        }
    }
}
