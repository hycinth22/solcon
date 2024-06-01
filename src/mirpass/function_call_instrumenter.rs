use crate::monitors_finder::MonitorsInfo;
use rustc_span::def_id::DefId;
use rustc_middle::ty::Ty;
use rustc_middle::ty::TyCtxt;
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
            let mut our_call_args = args.iter().map(|arg| {
                rustc_span::source_map::Spanned {
                    node: match arg.node {
                        // 不能直接clone Operand::Move，因为我们会错误地提前move参数，应该由原来的函数调用move它，我们更改所有move为copy
                        Operand::Move(place) => {
                            // 注意点：
                            // 1.仅&T实现了Copy trait而&mut T没有Copy trait
                            // 2. Operand::Copy在drop elaboration前要求有Copy trait，之后则无此要求
                            // 3. 由于我们的pass运行在optimized_mir（MIR to Binaries阶段），而drop elaboration在此之前的analysis阶段进行，
                            // 所以我们在此处可以对任意类型的变量进行Copy
                            Operand::Copy(place)
                        },
                        Operand::Copy(..) | Operand::Constant(..) => arg.node.clone(),
                    },
                    span: arg.span.clone(),
                }
            }).collect();
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
            let mut our_call_args : Vec<_> = args.iter().map(|arg| {
                rustc_span::source_map::Spanned {
                    node: match arg.node {
                        Operand::Move(place) => Operand::Move(place),
                        Operand::Copy(..) | Operand::Constant(..) => arg.node.clone(),
                    },
                    span: arg.span.clone(),
                }
            }).collect();
            our_call_args.push(rustc_span::source_map::Spanned{
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
                // 临时解决方案，阻止原函数调用的操作数move，而等待由我们的after函数去处理
                args: args.iter().map(|arg| {
                    rustc_span::source_map::Spanned {
                        node: match arg.node {
                            Operand::Move(place) => Operand::Copy(place),
                            Operand::Copy(..) | Operand::Constant(..) => arg.node.clone(),
                        },
                        span: arg.span.clone(),
                    }
                }).collect(),
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
