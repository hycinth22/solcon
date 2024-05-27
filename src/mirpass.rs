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
use rustc_session::cstore::CrateDepKind;
use rustc_span::{
    def_id::{CrateNum, DefId, DefIndex, LOCAL_CRATE},
    DUMMY_SP,
};

pub(crate) use crate::utils;
use crate::monitors::{self, try_match_with_our_function, MonitorsInfo};

mod test_target_handler;
mod mutex_handler;

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
            if crate_name.as_str() != "this_is_our_monitor_function" {
                info!("skip mismatch crate name {crate_name}");
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
                    try_match_with_our_function(tcx, &def_id, &mut monitors);
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
        debug!("found body instance of {}", def_path_str);

        if tcx.is_foreign_item(def_id) {
            // 跳过外部函数(例如 extern "C")
            trace!("skip body instance of {} because is_foreign_item", def_path_str);
            continue;
        }
        // Skip promoted src
        if body.source.promoted.is_some() {
            trace!("skip body instance of {} because promoted.is_some", def_path_str);
            continue;
        }
        if is_filtered_def_path(tcx, &def_path) {
            trace!("skip body instance of {:?} because utils::is_filtered_def_path", def_path_str);
            continue;
        }
        // dont know why enable here leads to undefined symbol. unfinished
        // if !tcx.is_codegened_item(def_id) {
        //     warn!("skip body instance of {:?} because not is_codegened_item", def_path_str);
        //     continue;
        // }
        debug!("visiting function body of {}", def_path_str);
        inject_for_bb(tcx, body, &monitors);
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
        return true;
    } else {
        debug!("unfiltered crate_name {crate_name_str}");
    }
    false
}

fn inject_for_bb<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx mut Body<'tcx>, monitors: &MonitorsInfo) {
    // 遍历基本块
    let bbs = body.basic_blocks.as_mut();
    let mut insert_before_call = HashMap::new();
    let _original_callinfo = Option::<TerminatorKind>::None;
    for (block, block_data) in bbs.iter_enumerated_mut() {
        let this_terminator = block_data.terminator_mut();
        let kind = &mut this_terminator.kind;
        if let TerminatorKind::Call { func, ..} = kind {
            let func_def_path_str = utils::get_function_path_str(tcx, &body.local_decls, &func);
            let func_def_path = utils::get_function_path(tcx, &body.local_decls, &func);
            if func_def_path_str.is_none() {
                warn!("Found call to function but fail to get function DefPath");
                continue;
            }
            let func_def_path_str = func_def_path_str.unwrap();
            debug!("Found call to function: {:?}", func_def_path_str);
            match func_def_path_str.as_str() {
                "this_is_our_test_target_mod::this_is_our_test_target_function"  => {
                    info!("Found call to this_is_our_test_target_function: {:?} (should instrument before)", func_def_path_str);
                    if let Some(before_fn) = monitors.test_target_before_fn {
                        let insertblocks = test_target_handler::add_before_handler(tcx, &mut body.local_decls, monitors, this_terminator, block, before_fn);
                        insert_before_call.extend(insertblocks);
                    } else {
                        warn!("monitors.test_target_before_fn.is_none");
                    }
                }
                "std::sync::Mutex::<T>::lock" => {
                    info!("Found call to mutex lock: {:?}  (should instrument before)", func_def_path_str);
                    let insertblocks = mutex_handler::add_mutex_lock_before_handler(tcx, &mut body.local_decls, monitors, this_terminator, block);
                    insert_before_call.extend(insertblocks);
                }
                _ => {}
            }
        }
    }
    for (origin_block, newblockdata) in insert_before_call.into_iter() {
        let newblockindex = bbs.push(newblockdata);
        if let TerminatorKind::Call { target, .. } = &mut bbs[origin_block].terminator_mut().kind {
            *target = Some(newblockindex);
        } else {
            panic!("all terminiator ins insertBeforeCall must be TerminatorKind::Call")
        }
    }

    let mut insert_after_call = HashMap::new();
    for (block, block_data) in bbs.iter_enumerated_mut() {
        let this_terminator = block_data.terminator_mut();
        let kind = &mut this_terminator.kind;
        if let TerminatorKind::Call { func, ..} = kind {
            let func_def_path_str = utils::get_function_path_str(tcx, &body.local_decls, &func);
            let func_def_path = utils::get_function_path(tcx, &body.local_decls, &func);
            if func_def_path.is_none() {
                warn!("Found call to function but fail to get function DefPath");
                continue;
            }
            let func_def_path_str = func_def_path_str.unwrap();
            debug!("Found call to function: {:?}", func_def_path_str);
            match func_def_path_str.as_str() {
                "this_is_our_test_target_mod::this_is_our_test_target_function" => {
                    info!("Found call to this_is_our_test_target_function: {:?} (should instrument after)", func_def_path_str);
                    if let Some(after_fn) = monitors.test_target_after_fn {
                        let insertblocks = test_target_handler::add_after_handler(tcx, &mut body.local_decls, monitors, this_terminator, block, after_fn);
                        insert_after_call.extend(insertblocks);
                    } else {
                        warn!("monitors.test_target_after_fn.is_none");
                    }
                }
                "std::sync::Mutex::<T>::lock" => {
                    info!("Found call to mutex lock: {:?}  (should instrument after)", func_def_path_str);
                    let insertblocks = mutex_handler::add_mutex_lock_after_handler(tcx, &mut body.local_decls, monitors, this_terminator, block);
                    insert_after_call.extend(insertblocks);
                }
                _ => {}
            }
        }
    }
    for (origin_block, newblockdata) in insert_after_call.into_iter() {
        let newblockindex = bbs.push(newblockdata);
        if let TerminatorKind::Call { target, .. } = &mut bbs[origin_block].terminator_mut().kind {
            *target = Some(newblockindex);
        } else {
            panic!("all terminiator ins insertAfterCall must be TerminatorKind::Call")
        }
    }
}

