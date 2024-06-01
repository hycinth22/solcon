use std::collections::HashMap;

use tracing::{trace, debug, info, warn, error};
use rustc_hir::BodyOwnerKind;
use rustc_hir::definitions::DefPath;
use rustc_hir::def::DefKind;
use rustc_metadata::creader::CStore;
use rustc_middle::mir::{*};
use rustc_middle::ty::{self, GenericArgs, Instance, Ty, TyCtxt};
use rustc_middle::middle::exported_symbols::ExportedSymbol;
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::mir::ConstOperand;
use rustc_middle::mir::patch::MirPatch;
use rustc_session::cstore::CrateDepKind;
use rustc_span::{
    def_id::{CrateNum, DefId, DefIndex, LOCAL_CRATE},
    DUMMY_SP,
};

pub(crate) use crate::utils;
use crate::monitors_finder::{self, MonitorsFinder, MonitorsInfo};
use crate::config;

mod test_target_handler;
mod mutex_lock_handler;
mod mutexguard_drop;

pub trait OurMirPass {
    fn run_pass<'tcx>(&self, 
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>, monitors: &MonitorsInfo)
    -> Option< MirPatch<'tcx> >;
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
        }
        None
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
        let call = &terminator.kind;
        if let TerminatorKind::Call { func, args, destination, target, unwind, call_source, fn_span} = call {
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
            let ty_dest = body.local_decls[destination.local].ty;
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
        }
        None
    }

}

pub trait ObjectDropInstrumenter {
    fn target_ty(&self) -> &'static str;
    fn add_before_handler<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, local_decls: &mut rustc_index::IndexVec<Local, LocalDecl<'tcx>>, 
        this_terminator: &mut Terminator<'tcx>, block: rustc_middle::mir::BasicBlock, 
        monitors: &MonitorsInfo,
    ) -> Option< MirPatch<'tcx> >;
    fn add_after_handler<'tcx>(&self, 
        tcx: TyCtxt<'tcx>, local_decls: &mut rustc_index::IndexVec<Local, LocalDecl<'tcx>>, 
        this_terminator: &mut Terminator<'tcx>, block: rustc_middle::mir::BasicBlock, 
        monitors: &MonitorsInfo,
    ) -> Option< MirPatch<'tcx> >;
}


pub fn run_our_pass<'tcx>(tcx: TyCtxt<'tcx>) {
    info!("our pass is running");
    info!("prescaning");
    let mut monitors = MonitorsInfo::default();
    let crates = tcx.crates(());
    let crate_store_untracked = tcx.cstore_untracked();
    let crate_store = crate_store_untracked
        .as_any()
        .downcast_ref::<CStore>()
        .unwrap();
    for krate in crates {
        let krate = krate.clone();
        if krate != LOCAL_CRATE {
            let crate_name = tcx.crate_name(krate);
            if crate_name.as_str() != config::MONITORS_LIB_CRATE_NAME {
                debug!("skip mismatch crate name {crate_name}");
                continue;
            }
            let crate_dep_kind = tcx.dep_kind(krate);
            info!("traversaling crate {}", crate_name);
            // Only public-facing way to traverse all the definitions in a non-local crate.
            // inspired by hacspec(https://github.com/rust-lang/rust/pull/85889)
            let crate_num_def_ids = crate_store.num_def_ids_untracked(krate);
            let def_ids = (0..crate_num_def_ids).into_iter().map(|id| DefId {
                krate: krate,
                index: DefIndex::from_usize(id),
            });
            for def_id in def_ids {
                let def_path_str = tcx.def_path_str(def_id);
                let def_kind = tcx.def_kind(def_id);
                info!("found external {def_kind:?} : {}", def_path_str);
                if matches!(def_kind, DefKind::Fn) {
                    monitors.try_match_with_our_function(tcx, &def_id);
                }
            }
        }
    }
    info!("{monitors:#?}");
    let all_function_local_def_ids = tcx.mir_keys(()); // all the body owners, but also things like struct constructors.
    for local_def_id in all_function_local_def_ids {
        let def_id = local_def_id.to_def_id();
        let def_path = tcx.def_path(def_id);
        let def_path_str = tcx.def_path_str(def_id);
        let body_owner_kind = tcx.hir().body_owner_kind(*local_def_id);
        match body_owner_kind {
            BodyOwnerKind::Const{..} | BodyOwnerKind::Static(..) => {
                warn!("skip body kind {:?}", body_owner_kind);
                return;
            }
            BodyOwnerKind::Fn | BodyOwnerKind::Closure => {}
        }

        // since the compiler doesnt provide mutable interface, using unsafe to get one from optimized_mir
        #[allow(invalid_reference_casting)]
        let body: &mut _ =  unsafe{
            let immutable_ref = tcx.optimized_mir(def_id);
            let mutable_ptr = immutable_ref as *const Body as *mut Body;
            &mut *mutable_ptr
        };
        // let def_id = body.source.def_id();
        trace!("found body instance of {}", def_path_str);

        if tcx.is_foreign_item(def_id) {
            // 跳过外部函数(例如 extern "C")
            debug!("skip body instance of {} because is_foreign_item", def_path_str);
            continue;
        }
        // Skip promoted src
        if body.source.promoted.is_some() {
            debug!("skip body instance of {} because promoted.is_some", def_path_str);
            continue;
        }
        if is_filtered_def_path(tcx, &def_path) {
            debug!("skip body instance of {:?} because utils::is_filtered_def_path", def_path_str);
            continue;
        }
        // dont know why enable here leads to undefined symbol. unfinished
        // if !tcx.is_codegened_item(def_id) {
        //     warn!("skip body instance of {:?} because not is_codegened_item", def_path_str);
        //     continue;
        // }
        debug!("try inject for bb of function body of {}", def_path_str);
        inject_for_body(tcx, body, &monitors, &[
            &test_target_handler::TestTargetCallHandler::default(),
            &mutex_lock_handler::MutexLockCallHandler::default(), 
        ], &[
            &mutexguard_drop::MutexGuardDropPass{},
        ]);
    }
}

fn is_filtered_def_path(tcx: TyCtxt<'_>, def_path: &DefPath) -> bool {
    is_filtered_crate(tcx, &def_path.krate)
}

fn is_filtered_crate(tcx: TyCtxt<'_>, krate: &CrateNum) -> bool {
    if tcx.is_panic_runtime(*krate) || tcx.is_compiler_builtins(*krate)
    {
        return true;
    }
    let crate_name = tcx.crate_name(*krate);
    let crate_name_str = crate_name.as_str();
    const FILTERED_CRATES: [&str; 27] = [
        // from rustc library(s)
        "alloc",
        "backtrace",
        "core",
        "panic_abort",
        "panic_unwind",
        "portable-simd",
        "proc_macro",
        "profiler_builtins",
        "rtstartup",
        "rustc_std_workspace_alloc",
        "rustc_std_workspace_core",
        "rustc_std_workspace_std",
        "std",
        "stdarch",
        "sysroot",
        "test",
        "unwind",
        // other common underlying libraries
        "adler",
        "addr2line",
        "gimli",
        "object",
        "memchr",
        "miniz_oxide",
        "hashbrown",
        "rustc_demangle",
        "std_detect",
        "libc"
    ];
    if FILTERED_CRATES.contains(&crate_name_str) {
        debug!("filtered crate_name {crate_name_str}");
        return true;
    } else {
        debug!("unfiltered crate_name {crate_name_str}");
    }
    false
}

fn inject_for_body<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx mut Body<'tcx>, monitors: &MonitorsInfo, 
    function_call_instrumenters: &[&dyn FunctionCallInstrumenter],
    our_passes: &[&dyn OurMirPass],
) {
    // Execute function call instrumenters
    let mut instruement_pos = Vec::new();
    for (bb, bb_data) in body.basic_blocks.iter_enumerated() {
        let terminator = bb_data.terminator();
        if let TerminatorKind::Call { func, ..} = &terminator.kind {
            // let func_def_path = utils::get_function_path(tcx, &body.local_decls, &func);
            let Some(func_def_path_str) = utils::get_function_path_str(tcx, &body.local_decls, &func) else {
                warn!("Found call to function but fail to get function DefPath");
                continue;
            };
            debug!("Found call to function: {:?}", func_def_path_str);
            for instrumenter in function_call_instrumenters.iter() {
                let target_function = instrumenter.target_function();
                if func_def_path_str == target_function {
                    let caller_def_id = body.source.def_id();
                    let caller_def_path_str = tcx.def_path_str(caller_def_id);
                    info!("Found call to {} in {:?}  (should instrumented)", target_function, func_def_path_str);
                    instruement_pos.push((bb, instrumenter, caller_def_id));
                }
            }
        }
    }
    for (bb, instrumenter, caller_def_path_str) in instruement_pos.into_iter() {
        let target_function = instrumenter.target_function();
        let mut loc_bb = bb;
        info!("Instrumenting before handler for call to function {} in {:?}", target_function, caller_def_path_str);
        if let Some((patch, moved_new_block)) = instrumenter.instrument_call_before(tcx, body, monitors, loc_bb) {
            loc_bb = moved_new_block;
            patch.apply(body);
        };
        info!("Instrumenting after handler for call to function {} in {:?}", target_function, caller_def_path_str);
        if let Some((patch, _moved_new_block)) = instrumenter.instrument_call_after(tcx, body, monitors, loc_bb) {
            patch.apply(body);
        }
    }

    // Execute our passes
    for p in our_passes.iter() {
        let patch = p.run_pass(tcx, body, monitors);
        if let Some(patch) = patch {
            patch.apply(body);
        }
    }
}

