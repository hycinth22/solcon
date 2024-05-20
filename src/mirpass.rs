use std::collections::HashMap;

use log::trace;
use rustc_hir::def_id::DefId;
use rustc_middle::mir::{*};
use rustc_middle::ty::{self, GenericArgs, Instance, Ty, TyCtxt};
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::mir::ConstOperand;
use rustc_span::DUMMY_SP;

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
        let name_path_str = tcx.def_path_str(def_id);
        trace!("found body instance of {}", name_path_str);
        if tcx.is_foreign_item(def_id) {
            // 跳过外部函数(例如 extern "C"{} )
            trace!("skip body instance of {} because is_foreign_item", name_path_str);
            continue;
        }
        // Skip promoted src
        if body.source.promoted.is_some() {
            trace!("skip body instance of {} because promoted.is_some", name_path_str);
            continue;
        }
        if filtered_function_body(name_path_str.as_str()) {
            trace!("skip body instance of {} because filtered_function_body", name_path_str);
            continue;
        }
        println!("visiting function body of {}", name_path_str);
        inject_for_bb(tcx, body, &info);
    }
}


fn filtered_function_body(fn_defpath_str: &str) -> bool {
    return fn_defpath_str.starts_with("std::") || fn_defpath_str.starts_with("core::" );
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

fn alloc_unit_local<'tcx>(tcx: TyCtxt<'tcx>, local_decls: &mut rustc_index::IndexVec<Local, LocalDecl<'tcx>>) -> Local {
    let local_decl = LocalDecl::new(tcx.types.unit, DUMMY_SP);
    let new_local= local_decls.push(local_decl);
    return new_local
}


fn inject_for_bb<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx mut Body<'tcx>, prescan_info: &search_monitor::PreScanInfo) {
    // 遍历基本块
    let bbs = body.basic_blocks.as_mut();
    let mut insert_before_call = HashMap::new();
    let _original_callinfo = Option::<TerminatorKind>::None;
    for (block, block_data) in bbs.iter_enumerated_mut() {
        let this_terminator = block_data.terminator_mut();
        let kind = &mut this_terminator.kind;
        if let TerminatorKind::Call { func, ..} = kind {
            let func_path: Option<String> = get_function_path(tcx, &body.local_decls, &func);
            if func_path.is_none() {
                println!("Found call to function but fail to get function path");
                continue;
            }
            let func_path = func_path.unwrap();
            // println!("found function call: {:?}", func_path);
            println!("Found call to function: {:?}", func_path);

            match func_path.as_str() {
                "this_is_our_test_target_mod::this_is_our_test_target_function" => {
                    println!("Found call to this_is_our_test_target_function: {:?}", func_path);
                    if let Some(before_fn) = prescan_info.test_target_before_fn {
                        let insertblocks = test_target_handler::add_before_handler(tcx, &mut body.local_decls, prescan_info, this_terminator, block, before_fn);
                        insert_before_call.extend(insertblocks);
                    } else {
                        println!("prescan_info.test_target_before_fn.is_none");
                    }
                }
                "std::sync::Mutex::<T>::lock" => {
                    println!("Found call to mutex lock: {:?}", func_path);
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
            let func_path: Option<String> = get_function_path(tcx, &body.local_decls, &func);
            if func_path.is_none() {
                println!("Found call to function but fail to get function path");
                continue;
            }
            let func_path = func_path.unwrap();
            // println!("found function call: {:?}", func_path);
            println!("Found call to function: {:?}", func_path);

            match func_path.as_str() {
                "this_is_our_test_target_mod::this_is_our_test_target_function" => {
                    println!("Found call to this_is_our_test_target_function: {:?}", func_path);
                    if let Some(after_fn) = prescan_info.test_target_after_fn {
                        let insertblocks = test_target_handler::add_after_handler(tcx, &mut body.local_decls, prescan_info, this_terminator, block, after_fn);
                        insert_after_call.extend(insertblocks);
                    } else {
                        println!("prescan_info.test_target_before_fn.is_none");
                    }
                }
                "std::sync::Mutex::<T>::lock" => {
                    println!("Found call to mutex lock: {:?}", func_path);
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
            if func_path == "this_is_our_monitor_function::this_is_our_test_target_function" {
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


fn get_function_path<'tcx, 'operand>(tcx: TyCtxt<'tcx>, local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &'operand Operand<'tcx>) -> Option<String> {
    // 通过Operand获取函数调用的名称
    return get_function_path_from_ty(tcx, &get_operand_ty( local_decls, operand));
}

fn get_function_generic_args<'tcx, 'operand>(local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &'operand Operand<'tcx>) -> Option<&'tcx GenericArgs<'tcx>> {
    // 通过Operand获取函数调用的GenericArg
    return get_function_generic_args_from_ty(&get_operand_ty(local_decls, operand));
}

fn get_operand_ty<'tcx>(local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &Operand<'tcx>) -> Ty<'tcx> {
    match operand {
        Operand::Constant(box ConstOperand { const_, .. }) => {
            match const_ {
                Const::Ty( ty_const) => {
                    //println!("ty!!");
                    return ty_const.ty();
                }
                Const::Unevaluated(_val, ty) => {
                    //println!("Unevaluated!!");
                    return ty.clone();
                }
                Const::Val( _val, ty) => {
                    //dbg!(_val);
                    //println!("Val!!");
                    return ty.clone();
                }
            }
        }
        Operand::Copy(place) | Operand::Move(place) => {
            let ty = local_decls[place.local].ty;
            // println!("Copy | Move !!");
            return ty;
        }
    }
}

fn get_function_generic_args_from_ty<'tcx>(ty: &ty::Ty<'tcx>) -> Option<&'tcx GenericArgs<'tcx>> {
    let ty_kind: &rustc_type_ir::TyKind<TyCtxt> = ty.kind();
    match ty_kind {
        ty::TyKind::FnDef(_def_id, args) | ty::Closure(_def_id, args) => {
            return Some(&args);
        }
        ty::FnPtr(_) => {
            println!("get_function_args_from_ty: FnPtr failed!!!!!!!!!!!!!!");   
        },
        ty::TyKind::Dynamic(_, _, _) => todo!(),
        ty::TyKind::CoroutineClosure(_, _) => todo!(),
        ty::TyKind::Coroutine(_, _) => unimplemented!(),
        ty::TyKind::CoroutineWitness(_, _) => todo!(),
        // the following all looks unlikely, but remains different branchs for debug
        ty::TyKind::Bool => todo!(),
        ty::TyKind::Char => todo!(),
        ty::TyKind::Int(_) => todo!(),
        ty::TyKind::Uint(_) => todo!(),
        ty::TyKind::Float(_) => todo!(),
        ty::TyKind::Adt(_, _) => todo!(),
        ty::TyKind::Foreign(_) => todo!(),
        ty::TyKind::Str => todo!(),
        ty::TyKind::Array(_, _) => todo!(),
        ty::TyKind::Pat(_, _) => todo!(),
        ty::TyKind::Slice(_) => todo!(),
        ty::TyKind::RawPtr(_, _) => todo!(),
        ty::TyKind::Ref(_, _, _) => todo!(),
        ty::TyKind::Never => todo!(),
        ty::TyKind::Tuple(_) => todo!(),
        ty::TyKind::Alias(_, _) => todo!(),
        ty::TyKind::Param(_) => todo!(),
        ty::TyKind::Bound(_, _) => todo!(),
        ty::TyKind::Placeholder(_) => todo!(),
        ty::TyKind::Infer(_) => todo!(),
        ty::TyKind::Error(_) => todo!(),
    }
    println!("get_function_path_from_ty: failed!!!!!!!!!!!!!!");
    None
}

fn get_function_path_from_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: &ty::Ty<'tcx>) -> Option<String> {
    let ty_kind = ty.kind();
    match ty_kind {
        ty::TyKind::FnDef(def_id, _args) | ty::Closure(def_id, _args) => {
            let func_path_with_args = tcx.def_path_str_with_args(def_id, _args);
            //dbg!(_args);
            //println!("get_function_path_from_ty func_path_with_args: {}", func_path_with_args);
            let func_path = tcx.def_path_str(*def_id);
            return Some(func_path);
        }
        ty::FnPtr(_) => {
            println!("get_function_path_from_ty: FnPtr failed!!!!!!!!!!!!!!");   
        },
        ty::TyKind::Dynamic(_, _, _) => todo!(),
        ty::TyKind::CoroutineClosure(_, _) => todo!(),
        ty::TyKind::Coroutine(_, _) => unimplemented!(),
        ty::TyKind::CoroutineWitness(_, _) => todo!(),
        // the following all looks unlikely, but remains different branchs for debug
        ty::TyKind::Bool => todo!(),
        ty::TyKind::Char => todo!(),
        ty::TyKind::Int(_) => todo!(),
        ty::TyKind::Uint(_) => todo!(),
        ty::TyKind::Float(_) => todo!(),
        ty::TyKind::Adt(_, _) => todo!(),
        ty::TyKind::Foreign(_) => todo!(),
        ty::TyKind::Str => todo!(),
        ty::TyKind::Array(_, _) => todo!(),
        ty::TyKind::Pat(_, _) => todo!(),
        ty::TyKind::Slice(_) => todo!(),
        ty::TyKind::RawPtr(_, _) => todo!(),
        ty::TyKind::Ref(_, _, _) => todo!(),
        ty::TyKind::Never => todo!(),
        ty::TyKind::Tuple(_) => todo!(),
        ty::TyKind::Alias(_, _) => todo!(),
        ty::TyKind::Param(_) => todo!(),
        ty::TyKind::Bound(_, _) => todo!(),
        ty::TyKind::Placeholder(_) => todo!(),
        ty::TyKind::Infer(_) => todo!(),
        ty::TyKind::Error(_) => todo!(),
    }
    println!("get_function_path_from_ty: failed!!!!!!!!!!!!!!");
    None
}