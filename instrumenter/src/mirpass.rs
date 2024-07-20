use tracing::{trace, debug, info, warn};
use rustc_hir::BodyOwnerKind;
use rustc_hir::definitions::DefPath;
use rustc_hir::def::DefKind;
use rustc_metadata::creader::CStore;
use rustc_middle::mir::{*};
use rustc_middle::ty::{TyCtxt, TyKind};
use rustc_middle::mir::patch::MirPatch;
use rustc_span::def_id::{CrateNum, DefId, DefIndex, LocalDefId, LOCAL_CRATE};

pub(crate) use crate::utils;
use crate::monitors_finder::{MonitorsFinder, MonitorsInfo};
use crate::{config, mem_instrumenter};

use crate::function_call_instrumenter;
pub use function_call_instrumenter::FunctionCallInstrumenter;
use crate::obj_drop_instrumenter;
pub use obj_drop_instrumenter::ObjectDropInstrumenter;

#[cfg(feature = "enable_debug_passes")]
mod debug_use_test_target_handler;
#[cfg(feature = "enable_debug_passes")]
mod debug_use_inspect_func_call;

mod mutex_lock_handler;
mod mutex_try_lock_handler;
mod mutexguard_drop_handler;
mod rwlock_read_handler;
mod rwlock_try_read_handler;
mod rwlock_readguard_drop_handler;
mod rwlock_write_handler;
mod rwlock_try_write_handler;
mod rwlock_writeguard_drop_handler;
mod barrier_wait_handler;
mod condvar_wait_handler;
mod condvar_wait_timeout_handler;
mod condvar_wait_timeout_ms_handler;
mod condvar_wait_while_handler;
mod condvar_wait_timeout_while_handler;
mod entry_fn_handler;

pub trait OurMirPass {
    fn run_pass<'tcx>(&self, 
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>, monitors: &MonitorsInfo)
    -> Option< MirPatch<'tcx> >;
}

pub(crate) static START_INSTRUMENT : std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(crate) static MONITORS : std::sync::OnceLock<MonitorsInfo> = std::sync::OnceLock::new();
pub(crate) static ENTRY_FN_DEF_ID : std::sync::OnceLock<DefId> = std::sync::OnceLock::new();

// call only once
pub fn find_all_monitors(tcx: TyCtxt<'_>) {
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
                trace!("skip mismatch crate name {crate_name}");
                continue;
            }
            let crate_dep_kind = tcx.dep_kind(krate);
            info!("traversaling crate {} ({crate_dep_kind:?})", crate_name);
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
                trace!("found external {def_kind:?} : {}", def_path_str);
                if matches!(def_kind, DefKind::Fn) {
                    monitors.try_match_with_our_function(tcx, &def_id);
                }
            }
        }
    }
    info!("{monitors:#?}");
    MONITORS.set(monitors).unwrap()
}

fn original_optimized_mir(tcx: TyCtxt<'_>, did: LocalDefId) -> &Body<'_> {
    let original_providers : _ = {
        let mut provider = rustc_middle::util::Providers::default();
        rustc_mir_transform::provide(&mut provider);
        provider
    };
    let original_optimized_mir = original_providers.optimized_mir;
    original_optimized_mir(tcx, did)
}

unsafe fn get_mut_ref_body<'bodyref, 'tcx>(body: &Body<'tcx>) -> &'bodyref mut Body<'tcx> {
    #[allow(invalid_reference_casting)]
    let body_mut: &mut _ =  unsafe{
        let immutable_ref= body;
        let mutable_ptr = immutable_ref as *const Body as *mut Body;
        &mut *mutable_ptr
    };
    body_mut
}

/// The original query is "Optimize the MIR and prepare it for codegen."
/// here We instrument mir::Body
pub(crate) fn our_optimized_mir(tcx: TyCtxt<'_>, did: LocalDefId) -> &Body<'_> {
    let body = original_optimized_mir(tcx, did);
    if START_INSTRUMENT.load(std::sync::atomic::Ordering::Acquire) {
        // we should be the only reference holder in the window from the end of the original query until the return, according to the current implementation of optimized_mir .
        let body_mut = unsafe {get_mut_ref_body(body)};
        let monitors = MONITORS.get().unwrap();
        if let Some(entry_fn_def_id) = ENTRY_FN_DEF_ID.get() {
            let entry_fn_local_def_id = entry_fn_def_id.expect_local();
            if entry_fn_local_def_id == did {
                let entry_fn_handler = entry_fn_handler::EntryFnBodyInstrumenter::default();
                entry_fn_handler.instrument_body(tcx, body_mut, monitors);
            }
        }
        run_our_pass_on_body(tcx, &monitors, did, body_mut);
    }
    body
}

pub fn find_entry_fn(tcx: TyCtxt<'_>) {
    let Some((entry_fn, entry_fn_type)) = tcx.entry_fn(()) else {
        info!("skip instruement_entry_fn because tcx.entry_fn(()) is None");
        return;
    };
    let entry_fn_def_path_str = tcx.def_path_str(entry_fn);
    info!("found entry_fn {entry_fn_def_path_str}, type {entry_fn_type:?}");
    ENTRY_FN_DEF_ID.set(entry_fn).unwrap();
}

pub fn run_our_pass_on_body<'tcx>(tcx: TyCtxt<'tcx>, monitors: &MonitorsInfo,
  local_def_id:LocalDefId, body: &mut Body<'tcx>
) {
    let def_id = local_def_id.to_def_id();
    let def_path = tcx.def_path(def_id);
    let def_path_str = tcx.def_path_str(def_id);
    let body_owner_kind = tcx.hir().body_owner_kind(local_def_id);
    match body_owner_kind {
        BodyOwnerKind::Const{..} | BodyOwnerKind::Static(..) => {
            warn!("skip body kind {:?}", body_owner_kind);
            return;
        }
        BodyOwnerKind::Fn | BodyOwnerKind::Closure => {}
    }
    trace!("found body instance of {}", def_path_str);

    if tcx.is_foreign_item(def_id) {
        // 跳过外部函数(例如 extern "C")
        debug!("skip body instance of {} because is_foreign_item", def_path_str);
        return;
    }
    // Skip promoted src
    if body.source.promoted.is_some() {
        debug!("skip body instance of {} because promoted.is_some", def_path_str);
        return;
    }
    if is_filtered_def_path(tcx, &def_path) {
        debug!("skip body instance of {:?} because utils::is_filtered_def_path", def_path_str);
        return;
    }
    // dont know why enable here leads to undefined symbol. unfinished
    // if !tcx.is_codegened_item(def_id) {
    //     warn!("skip body instance of {:?} because not is_codegened_item", def_path_str);
    //     continue;
    // }
    info!("--------- running pass on function body of {}", def_path_str);
    inject_for_body(tcx, body, &monitors, &[
        #[cfg(feature = "enable_debug_passes")]
        &debug_use_test_target_handler::TestTargetCallHandler::default(),
        #[cfg(feature = "enable_debug_passes")]
        &debug_use_inspect_func_call::FunctionCallInspectorInstrumenter::default(),

        &mutex_lock_handler::MutexLockCallHandler::default(), 
        &mutex_try_lock_handler::MutexTryLockCallHandler::default(), 
        &rwlock_read_handler::RwLockReadCallHandler::default(), 
        &rwlock_write_handler::RwLockWriteCallHandler::default(), 
        &rwlock_try_read_handler::RwLockTryReadCallHandler::default(), 
        &rwlock_try_write_handler::RwLockTryWriteCallHandler::default(), 
        &barrier_wait_handler::BarrierWaitCallHandler::default(), 
        &condvar_wait_handler::CondvarWaitCallHandler::default(), 
        &condvar_wait_timeout_handler::CondvarWaitTimeoutCallHandler::default(), 
        &condvar_wait_timeout_ms_handler::CondvarWaitTimeoutMsCallHandler::default(), 
        &condvar_wait_while_handler::CondvarWaitWhileCallHandler::default(), 
        &condvar_wait_timeout_while_handler::CondvarWaitTimeoutWhileCallHandler::default(), 
    ],
    &[
        &mutexguard_drop_handler::MutexGuardDropInstrumenter::default(),
        &rwlock_readguard_drop_handler::RwLockReadGuardDropInstrumenter::default(),
        &rwlock_writeguard_drop_handler::RwLockWriteGuardDropInstrumenter::default(),
        
    ]
    , &[]);
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
    const FILTERED_CRATES: [&str; 28] = [
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
        "libc",
        "proc-macro-crate",
    ];
    if FILTERED_CRATES.contains(&crate_name_str) {
        debug!("filtered crate_name {crate_name_str}");
        return true;
    } else {
        trace!("unfiltered crate_name {crate_name_str}");
    }
    false
}

fn inject_for_body<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>, monitors: &MonitorsInfo, 
    function_call_instrumenters: &[&dyn FunctionCallInstrumenter],
    object_drop_instrumenters: &[&dyn ObjectDropInstrumenter],
    our_passes: &[&dyn OurMirPass],
) {
    // Instrument memory acesses
    mem_instrumenter::instrument_mem_acesses(tcx, body, monitors);
    // Execute function call instrumenters
    execute_all_function_call_instrumenters(tcx, body, monitors, function_call_instrumenters);
    // Execute object drop instrumenters
    execute_all_obj_drop_instrumenters(tcx, body, monitors, object_drop_instrumenters);
    // Execute our other passes
    for p in our_passes.iter() {
        let patch = p.run_pass(tcx, body, monitors);
        if let Some(patch) = patch {
            patch.apply(body);
        }
    }
}

fn execute_all_function_call_instrumenters<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>, 
monitors: &MonitorsInfo, function_call_instrumenters: &[&dyn FunctionCallInstrumenter]
) {
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
                    info!("Found call to {} in {:?}  (should instrumented)", target_function, caller_def_path_str);
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
}

fn execute_all_obj_drop_instrumenters<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>, 
monitors: &MonitorsInfo, object_drop_instrumenters: &[&dyn ObjectDropInstrumenter]) 
{
    let mut instruement_pos = Vec::new();
    for (bb, bb_data) in body.basic_blocks.iter_enumerated() {
        let terminator = bb_data.terminator();
        match &terminator.kind {
            TerminatorKind::Drop { place, ..} => {
                let ty = place.ty(&body.local_decls, tcx).ty;
                if let TyKind::Adt(adt_def, generic_args) = ty.kind() {
                    let ty_def_id = adt_def.did();
                    let ty_def_path_str = tcx.def_path_str(ty_def_id);
                    info!("found drop of type {}", ty_def_path_str);
                    for instrumenter in object_drop_instrumenters.iter() {
                        let target_ty = instrumenter.target_ty();
                        if ty_def_path_str == target_ty {
                            let caller_def_id = body.source.def_id();
                            let caller_def_path_str = tcx.def_path_str(caller_def_id);
                            info!("Found drop of {} in {:?}  (should instrumented)", target_ty, caller_def_path_str);
                            instruement_pos.push((bb, instrumenter, caller_def_id, generic_args));
                        }
                    }
                }
            }
            TerminatorKind::Call{ func, ..} => {
                let Some(func_def_path_str) = utils::get_function_path_str(tcx, &body.local_decls, &func) else {
                    warn!("Found call to function but fail to get function DefPath");
                    continue;
                };
                if func_def_path_str == "std::mem::drop" || func_def_path_str == "core::mem::drop" {
                    let Some(generic_args) = utils::get_function_generic_args(tcx, &body.local_decls, &func) else {
                        warn!("Found call to std/core::mem::drop but fail to get function generic_args");
                        continue;
                    };
                    let arg_ty = generic_args.type_at(0);
                    // according to https://doc.rust-lang.org/nightly/error_codes/E0120.html
                    // only structs, enums, and unions can implement Drop.
                    if let TyKind::Adt(adt_def, generic_args) = arg_ty.kind() {
                        let ty_def_id = adt_def.did();
                        let ty_def_path_str = tcx.def_path_str(ty_def_id);
                        info!("found call to drop function {func_def_path_str} for type {ty_def_path_str}");
                        for instrumenter in object_drop_instrumenters.iter() {
                            let target_ty = instrumenter.target_ty();
                            if ty_def_path_str == target_ty {
                                let caller_def_id = body.source.def_id();
                                let caller_def_path_str = tcx.def_path_str(caller_def_id);
                                info!("Found drop of {} in {:?}  (should instrumented)", target_ty, caller_def_path_str);
                                instruement_pos.push((bb, instrumenter, caller_def_id, generic_args));
                            }
                        }
                    } else {
                        warn!("found call to drop function {func_def_path_str} but type is not adt");
                    }
                }
                if func_def_path_str == "std::ptr::drop_in_place" || func_def_path_str == "core::ptr::drop_in_place" {
                        unimplemented!("unimplement process for ptr::drop_in_place");
                }
            }
            _ => {}
        }
    }
    for (bb, instrumenter, caller_def_path_str, _generic_args) in instruement_pos.into_iter() {
        let target_function = instrumenter.target_ty();
        let mut loc_bb = bb;
        info!("Instrumenting before handler for drop of type {} in {:?}", target_function, caller_def_path_str);
        if let Some((patch, moved_new_block)) = instrumenter.instrument_drop_before(tcx, body, monitors, loc_bb) {
            loc_bb = moved_new_block;
            patch.apply(body);
        };
        info!("Instrumenting after handler for drop of type {} in {:?}", target_function, caller_def_path_str);
        if let Some((patch, _moved_new_block)) = instrumenter.instrument_drop_after(tcx, body, monitors, loc_bb) {
            patch.apply(body);
        }
    }
}


