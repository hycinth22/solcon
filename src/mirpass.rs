use std::collections::HashMap;

use log::trace;
use rustc_hir::definitions::DefPath;
use rustc_hir::def_id::{CrateNum, DefId};
use rustc_middle::mir::{*};
use rustc_middle::ty::{self, GenericArgs, Instance, Ty, TyCtxt};
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::mir::ConstOperand;
use rustc_span::DUMMY_SP;

pub(crate) use crate::utils;

mod search_monitor;
mod test_target_handler;
mod mutex_handler;


pub fn run_our_pass<'tcx>(tcx: TyCtxt<'tcx>) {
    println!("our pass is running");
    // let (items, cgus) = tcx.collect_and_partition_mono_items(());
    // println!("cgus.len {}", cgus.len());
    // let instances: Vec<Instance<'tcx>> = cgus
    // .iter()
    // .flat_map(|cgu| {
    //     cgu.items().iter().filter_map(|(mono_item, _)| {
    //         if let MonoItem::Fn(instance) = mono_item {
    //             Some(*instance)
    //         } else {
    //             None
    //         }
    //     })
    // })
    // .collect();
    let all_function_local_def_ids = tcx.mir_keys(());
    println!("prescaning");
    let mut info = search_monitor::PreScanInfo::default();
    // for def_id in items.iter() {
    //     let body = tcx.optimized_mir(def_id);
    for local_def_id in all_function_local_def_ids {
        let def_id = local_def_id.to_def_id();
        let body = tcx.optimized_mir(def_id);
    // for instance in instances.iter() {
    //     let body = tcx.instance_mir(instance.def);
        search_monitor::try_match_with_our_function(tcx, body, &mut info);
    }
    dbg!(&info);
    // for instance in instances.iter() {
    //     let body = tcx.instance_mir(instance.def);
    //     find_our_function(tcx, instance, &mut info);
    // }
    // for instance in instances {
    //     #[allow(invalid_reference_casting)]
    //     let body = unsafe {
    //         let immutable_ref = tcx.instance_mir(instance.def);
    //         let mutable_ptr = immutable_ref as *const Body as *mut Body;
    //         &mut *mutable_ptr
    //    };
    for local_def_id in all_function_local_def_ids {
        let def_id = local_def_id.to_def_id();
        #[allow(invalid_reference_casting)]
        let body =  unsafe{
            let immutable_ref = tcx.optimized_mir(def_id);
            let mutable_ptr = immutable_ref as *const Body as *mut Body;
            &mut *mutable_ptr
        };
    //     let mirbody = tcx.mir_built(local_def_id);
    //     #[allow(invalid_reference_casting)]
    //     let mirbody = unsafe {
    //         let immutable_ref = mirbody;
    //         let mutable_ptr = immutable_ref as *const Steal<Body> as *mut Steal<Body>;
    //         &mut *mutable_ptr
    //    };
    //     let body = mirbody.get_mut();
    //     #[allow(invalid_reference_casting)]
    //     let body = unsafe {
    //         let immutable_ref = body;
    //         let mutable_ptr = immutable_ref as *const Body as *mut Body;
    //         &mut *mutable_ptr
    //    };
        let def_id = body.source.def_id();
        //assert!(tcx.is_codegened_item(def_id));
        let def_path = tcx.def_path(def_id);
        let def_path_str = tcx.def_path_str(def_id);
        println!("found body instance of {}", def_path_str);
        if tcx.is_foreign_item(def_id) {
            // 跳过外部函数(例如 extern "C"{} )
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
        println!("visiting function body of {}", def_path_str);
        inject_for_bb(tcx, body, &info);
    }
}

fn is_filtered_def_path(tcx: TyCtxt<'_>, def_path: &DefPath) -> bool {
    is_filtered_crate(tcx, &def_path.krate)
}

fn is_filtered_crate(tcx: TyCtxt<'_>, krate: &CrateNum) -> bool {
    let crate_name = tcx.crate_name(*krate);
    let crate_name_str = crate_name.as_str();
    const FILTERED_CRATES: [&str; 14] = [
        "alloc",
        "backtrace",
        "core",
        "panic_abort",
        "panic_unwind",
        "portable-simd",
        "proc_macro",
        "profiler_builtins",
        "rtstartup",
        "std",
        "stdarch",
        "sysroot",
        "test",
        "unwind",
    ];
    if FILTERED_CRATES.contains(&crate_name_str) {
        return true;
    } else {
        println!("unfiltered crate_name {crate_name_str}");
    }
    false
}

fn is_target_crate(tcx: TyCtxt<'_>, krate: &CrateNum, target_crates: &[&str]) -> bool {
    let crate_name = tcx.crate_name(*krate);
    let crate_name_str = crate_name.as_str();
    target_crates.contains(&crate_name_str)
}

// fn find_def_id_by_path(tcx: TyCtxt<'_>, def_path: &[&str]) -> Option<DefId> {
//     // 获取根 DefId
//     let root_def_id = tcx.hir().as_local_hir_id(0);

//     // 查找根 DefPath
//     let root_def_path = tcx.def_path(root_def_id).to_string();

//     // 比较根 DefPath 和目标 DefPath 的共同前缀
//     let mut common_prefix_len = 0;
//     while common_prefix_len < root_def_path.len() && common_prefix_len < def_path.len() {
//         if root_def_path[common_prefix_len] != def_path[common_prefix_len] {
//             break;
//         }
//         common_prefix_len += 1;
//     }

//     // 构造目标 DefPath
//     let mut target_def_path = root_def_path;
//     for segment in &def_path[common_prefix_len..] {
//         target_def_path.push_str("::");
//         target_def_path.push_str(segment);
//     }

//     // 查找目标 DefId
//     for &def_id in tcx.global_ctors().iter().chain(tcx.hir().krate().exports) {
//         let current_def_path = tcx.def_path(def_id);
//         if current_def_path.to_string() == target_def_path {
//             return Some(def_id);
//         }
//     }

//     None
// }

fn inject_for_bb<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx mut Body<'tcx>, prescan_info: &search_monitor::PreScanInfo) {
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
                println!("Found call to function but fail to get function DefPath");
                continue;
            }
            let func_def_path_str = func_def_path_str.unwrap();
            // println!("found function call: {:?}", func_def_path_str);
            println!("Found call to function: {:?}", func_def_path_str);
            if func_def_path_str.ends_with("::this_is_our_test_target_mod::this_is_our_test_target_function") {
                println!("Found foreigner's call to this_is_our_test_target_function: {:?}", func_def_path_str);
                if let Some(before_fn) = prescan_info.test_target_before_fn {
                    let insertblocks = test_target_handler::add_before_handler(tcx, &mut body.local_decls, prescan_info, this_terminator, block, before_fn);
                    insert_before_call.extend(insertblocks);
                } else {
                    println!("prescan_info.test_target_before_fn.is_none");
                }
            }
            match func_def_path_str.as_str() {
                "this_is_our_test_target_mod::this_is_our_test_target_function"  => {
                    println!("Found call to this_is_our_test_target_function: {:?}", func_def_path_str);
                    if let Some(before_fn) = prescan_info.test_target_before_fn {
                        let insertblocks = test_target_handler::add_before_handler(tcx, &mut body.local_decls, prescan_info, this_terminator, block, before_fn);
                        insert_before_call.extend(insertblocks);
                    } else {
                        println!("prescan_info.test_target_before_fn.is_none");
                    }
                }
                "std::sync::Mutex::<T>::lock" => {
                    println!("Found call to mutex lock: {:?}", func_def_path_str);
                    let insertblocks = mutex_handler::add_mutex_lock_before_handler(tcx, &mut body.local_decls, prescan_info, this_terminator, block);
                    insert_before_call.extend(insertblocks);
                }
                _ => {}
            }
        }
    }
    //let mut relocate_map = HashMap::new();
    for (origin_block, newblockdata) in insert_before_call.into_iter() {
        let newblockindex = bbs.push(newblockdata);
        if let TerminatorKind::Call { target, .. } = &mut bbs[origin_block].terminator_mut().kind {
            *target = Some(newblockindex);
            // 因为insertBeforeCall会影响原基本块，原函数调用是在新块运行，我们记录原块和新块的对应关系以便其他修改
            //relocate_map.insert(origin_block, newblockindex);
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
                println!("Found call to function but fail to get function DefPath");
                continue;
            }
            let func_def_path_str = func_def_path_str.unwrap();
            // println!("found function call: {:?}", func_def_path_str);
            println!("Found call to function: {:?}", func_def_path_str);

            if func_def_path_str.ends_with("::this_is_our_test_target_mod::this_is_our_test_target_function") {
                println!("Found foreigner's call to this_is_our_test_target_function: {:?}", func_def_path_str);
                if let Some(after_fn) = prescan_info.test_target_after_fn {
                    let insertblocks = test_target_handler::add_after_handler(tcx, &mut body.local_decls, prescan_info, this_terminator, block, after_fn);
                    insert_after_call.extend(insertblocks);
                } else {
                    println!("prescan_info.test_target_after_fn.is_none");
                }
            }
            match func_def_path_str.as_str() {
                "this_is_our_test_target_mod::this_is_our_test_target_function" => {
                    println!("Found call to this_is_our_test_target_function: {:?}", func_def_path_str);
                    if let Some(after_fn) = prescan_info.test_target_after_fn {
                        let insertblocks = test_target_handler::add_after_handler(tcx, &mut body.local_decls, prescan_info, this_terminator, block, after_fn);
                        insert_after_call.extend(insertblocks);
                    } else {
                        println!("prescan_info.test_target_after_fn.is_none");
                    }
                }
                "std::sync::Mutex::<T>::lock" => {
                    println!("Found call to mutex lock: {:?}", func_def_path_str);
                    let insertblocks = mutex_handler::add_mutex_lock_after_handler(tcx, &mut body.local_decls, prescan_info, this_terminator, block);
                    insert_after_call.extend(insertblocks);
                }
                _ => {}
            }
        }
    }
    

    for (origin_block, newblockdata) in insert_after_call.into_iter() {
        // let origin_block = {
        //      // 因为insertBeforeCall会影响原基本块，原函数调用是在新块运行，我们应该应用insertBeforeCall留下的修正信息
        //     if let Some(redirect_block) = relocate_map.remove(&origin_block) {
        //         redirect_block
        //     } else {
        //         origin_block
        //     }
        // };
        let newblockindex = bbs.push(newblockdata);
        if let TerminatorKind::Call { target, .. } = &mut bbs[origin_block].terminator_mut().kind {
            *target = Some(newblockindex);
        } else {
            panic!("all terminiator ins insertAfterCall must be TerminatorKind::Call")
        }
    }
}

/*
            if func_def_path_str == "this_is_our_monitor_function::this_is_our_test_target_function" {
                println!("detect our test target function, transforming");
                // 在函数调用之前插入我们的函数调用需要
                // 1 .更改当前块的terminator call的func到我们的函数，target到我们的新块以便返回后继续在新块执行原调用
                // 2. 把原函数调用移动到下一个我们新生成的基本块，terminator-kind为call，target到当前块的原target
                let ourfunc = func.clone();
                // this_terminator.target will be modify later because new block have not been inserted yet
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
                *func = ourfunc;
                *destination = Place::from(alloc_unit_local(tcx, &mut body.local_decls));
                insertBeforeCall.insert(block, bbdata);
                // 在函数调用之后插入我们的函数调用需要
                // 1 .更改当前块的terminator call的target到我们的新块
                // 2. 在我们新生成的基本块中，terminator-kind为call，func为我们的函数，target到当前块的原target
                // this_terminator.target will be modify later because new block have not been inserted yet
                let ourfunc = func.clone();
                let bbdata = BasicBlockData {
                    statements: vec![],
                    terminator: Some(Terminator {
                        kind: TerminatorKind::Call { 
                            func: ourfunc, 
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
                insertAfterCall.insert(block, bbdata);
            }
            else 
*/

