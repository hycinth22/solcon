use std::collections::HashMap;

use log::trace;
use rustc_data_structures::steal::Steal;
use rustc_hir::def_id::DefId;
use rustc_index::IndexVec;
use rustc_middle::mir::{self, *};
use rustc_middle::ty::{self, GenericArg, GenericArgs, Instance, Ty, TyCtxt, TyKind};
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::mir::ConstOperand;
use rustc_span::source_map::Spanned;
use rustc_span::{Span, DUMMY_SP};

#[derive(Default, Debug)]
struct PreScanInfo {
    mutex_lock_before_fn: Option<DefId>,
    mutex_lock_after_fn: Option<DefId>,
}

pub fn run_our_pass<'tcx>(tcx: TyCtxt<'tcx>) {
    println!("our pass is running");
    let cgus: &[mono::CodegenUnit] = tcx.collect_and_partition_mono_items(()).1;
    let instances: Vec<Instance<'tcx>> = cgus
    .iter()
    .flat_map(|cgu| {
        cgu.items().iter().filter_map(|(mono_item, _)| {
            if let MonoItem::Fn(instance) = mono_item {
                Some(*instance)
            } else {
                None
            }
        })
    })
    .collect();
    let all_function_local_def_ids = tcx.mir_keys(());
    println!("prescaning");
    let mut info = PreScanInfo::default();
    for local_def_id in all_function_local_def_ids {
        let def_id = local_def_id.to_def_id();
        let body = tcx.optimized_mir(def_id);
       find_our_function(tcx, body, &mut info);
    }
    dbg!(&info);
    // for instance in instances.iter() {
    //     let body = tcx.instance_mir(instance.def);
    //     find_our_function(tcx, instance, &mut info);
    // }
    for instance in instances {
        #[allow(invalid_reference_casting)]
        let body = unsafe {
            let immutable_ref = tcx.instance_mir(instance.def);
            let mutable_ptr = immutable_ref as *const Body as *mut Body;
            &mut *mutable_ptr
       };
    // for local_def_id in all_function_local_def_ids {
    //     let def_id = local_def_id.to_def_id();
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


fn find_our_function<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx Body<'tcx>, info: &mut PreScanInfo)  {
    let def_id: DefId = body.source.def_id();
    let fn_defpath_str = tcx.def_path_str(def_id);
    println!("find_our_function in {}", fn_defpath_str);
    if fn_defpath_str == "this_is_our_monitor_function::this_is_our_mutex_lock_before_handle_function" {
        info.mutex_lock_before_fn = Some(def_id);
        println!("configure info.mutex_lock_before_fn");
    } else if fn_defpath_str == "this_is_our_monitor_function::this_is_our_mutex_lock_after_handle_function" {
        info.mutex_lock_after_fn = Some(def_id);
        println!("configure info.mutex_lock_after_fn");
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


fn inject_for_bb<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx mut Body<'tcx>, prescan_info: &PreScanInfo) {
    // 遍历基本块
    let mut insertBeforeCall = HashMap::new();
    let mut insertAfterCall = HashMap::new();
    let mut bbs = body.basic_blocks.as_mut();
    let mut bbs_iter = bbs.iter_enumerated_mut();

    for (block, block_data) in bbs_iter {
        let mut this_terminator = block_data.terminator_mut();
        if let TerminatorKind::Call { func, args, destination, target, unwind, call_source, fn_span} = &mut this_terminator.kind {
            let func_path: Option<String> = get_function_path(tcx, &body.local_decls, &func);
            if func_path.is_none() {
                println!("Found call to function but fail to get function path");
                continue;
            }
            let func_path = func_path.unwrap();
            // println!("found function call: {:?}", func_path);
            println!("Found call to function: {:?}", func_path);
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
            else if func_path == "std::sync::Mutex::<T>::lock" {
                let oooorigin_args = args.clone();
                let generic_args = get_function_generic_args(tcx, &body.local_decls, &func);
                if generic_args.is_none() {
                    println!("generic_args.is_none");
                    continue;
                }
                let generic_args = generic_args.unwrap();
                println!("Found call to mutex lock: {:?}", func_path);
                // 在函数调用之前插入我们的函数调用需要
                // 1 .更改当前块的terminator call的func到我们的函数，target到我们的新块以便返回后继续在新块执行原调用
                // 2. 把原函数调用移动到下一个我们新生成的基本块，terminator-kind为call，target到当前块的原target
                let ourfunc = {
                    // let func_path = &["this_is_our_monitor_function", "this_is_our_mutex_lock_mock_function", "<T>"];
                    // let func_def_id = find_def_id_by_pat(tcx, func_path);
                    if prescan_info.mutex_lock_before_fn.is_none() {
                        println!("prescan_info.mutex_lock_before_fn.is_none");
                        continue;
                    }
                    let func_def_id = prescan_info.mutex_lock_before_fn.unwrap();
                    let func_ty = tcx.type_of(func_def_id).instantiate(tcx, generic_args);
                    let r = tcx.resolve_instance(tcx.param_env(func_def_id).and((func_def_id, generic_args)));
                    let const_ = mir::Const::zero_sized(func_ty);
                    Operand::Constant(Box::new(ConstOperand {
                        span: DUMMY_SP,
                        const_: const_,
                        user_ty: None,
                    }))

                    // Operand::Val(val, func_ty)
                };
                // this_terminator.target will be modify later because new block have not been inserted yet
                let our_args = {
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
                // backup original info
                let origin_destination = destination.clone();
                let origin_args = args.clone();
                // start to modify
                *func = ourfunc;
                *destination = Place::from(alloc_unit_local(tcx, &mut body.local_decls));
                for arg in  args.iter_mut() {
                    if let Operand::Move(place) = arg.node {
                        arg.node = Operand::Copy(place);
                    }
                }
                insertBeforeCall.insert(block, bbdata);



                // 在函数调用之后插入我们的函数调用需要
                // 1 .更改当前块的terminator call的target到我们的新块
                // 2. 在我们新生成的基本块中，terminator-kind为call，func为我们的函数，target到当前块的原target
                // this_terminator.target will be modify later because new block have not been inserted yet
                // 恢复一些前面修改的信息
                let destination: Place = origin_destination;
                let args = origin_args;

                let ourfunc = {
                    // let func_path = &["this_is_our_monitor_function", "this_is_our_mutex_lock_mock_function", "<T>"];
                    // let func_def_id = find_def_id_by_pat(tcx, func_path);
                    if prescan_info.mutex_lock_after_fn.is_none() {
                        println!("prescan_info.mutex_lock_after_fn.is_none");
                        continue;
                    }
                    let func_def_id = prescan_info.mutex_lock_after_fn.unwrap();
                    let func_ty = tcx.type_of(func_def_id).instantiate(tcx, generic_args);
                    let r: Result<Option<Instance>, rustc_span::ErrorGuaranteed> = tcx.resolve_instance(tcx.param_env(func_def_id).and((func_def_id, generic_args)));
                    let const_ = mir::Const::zero_sized(func_ty);
                    Operand::Constant(Box::new(ConstOperand {
                        span: DUMMY_SP,
                        const_: const_,
                        user_ty: None,
                    }))
                };
            
                // 为了传入返回值，先构造一条创建引用的statement并插到我们的函数调用前
                let ty_dest = body.local_decls[destination.local].ty;
                let local_decl = LocalDecl::new(Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, ty_dest), DUMMY_SP);
                let ref_dest= body.local_decls.push(local_decl);

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
 
                    // 临时解决方案，阻止原函数调用的操作数move，而我们的after函数去处理
                    if let TerminatorKind::Call { args, .. } = &mut insertBeforeCall.get_mut(&block).unwrap().terminator_mut().kind {
                        for arg in args {
                            if let Operand::Move(place) = arg.node {
                                arg.node = Operand::Copy(place);
                            }
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
                        span: DUMMY_SP,
                    });
                    our_args
                };
                let our_dest = Place::from(alloc_unit_local(tcx, &mut body.local_decls));
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
                insertAfterCall.insert(block, bbdata);
            }
            else if func_path == "<std::sync::MutexGuard<'_, T> as std::ops::Drop>::drop" {
                println!("Found call to MutexGuard drop: {:?}", func_path);
            }
            else if func_path == "std::sync::atomic::RwLock::<T>::read" {
                println!("Found call to RwLock read lock: {:?}", func_path);
            } 
            else if func_path == "std::sync::atomic::RwLock::<T>::write" {
                println!("Found call to RwLock write lock: {:?}", func_path);
            } 
            else if func_path.starts_with("std::sync::atomic::AtomicI32::store") {
                println!("Found call to atomic function: {:?}", func_path);
                println!("args: {:?}", args);
                
                if let rustc_middle::mir::Operand::Constant(box ConstOperand { const_, .. } ) = args[1].node {
                    if let rustc_middle::mir::Const::Val(rustc_middle::mir::ConstValue::Scalar( scalar ), ty) = const_ {
                        
                        if let rustc_middle::mir::interpret::Scalar::Int(mut scalar_int) = scalar {
                            scalar_int = 1234u32.into();
                            println!("yyy {:?}", scalar_int);
                        }
                    }

                    
                }
            }
        }

        
    }

    let mut relocate_map = HashMap::new();
    for (origin_block, newblockdata) in insertBeforeCall.into_iter() {
        let newblockindex = bbs.push(newblockdata);
        if let TerminatorKind::Call { target, .. } = &mut bbs[origin_block].terminator_mut().kind {
            *target = Some(newblockindex);

            // 因为insertBeforeCall会影响原基本块，原函数调用是在新块运行，我们记录原块和新块的对应关系以便其他修改
            relocate_map.insert(origin_block, newblockindex);
        } else {
            panic!("all terminiator ins insertBeforeCall must be TerminatorKind::Call")
        }
    }

    for (origin_block, newblockdata) in insertAfterCall.into_iter() {
        let origin_block = {
             // 因为insertBeforeCall会影响原基本块，原函数调用是在新块运行，我们应该应用insertBeforeCall留下的修正信息
            if let Some(redirect_block) = relocate_map.remove(&origin_block) {
                redirect_block
            } else {
                origin_block
            }
        };
        let newblockindex = bbs.push(newblockdata);
        if let TerminatorKind::Call { target, .. } = &mut bbs[origin_block].terminator_mut().kind {
            *target = Some(newblockindex);
        } else {
            panic!("all terminiator ins insertAfterCall must be TerminatorKind::Call")
        }
    }
}


fn get_function_path<'tcx, 'operand>(tcx: TyCtxt<'tcx>, local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &'operand Operand<'tcx>) -> Option<String> {
    // 通过Operand获取函数调用的名称
    return get_function_path_from_ty(tcx, &get_operand_ty(tcx, local_decls, operand));
}

fn get_function_generic_args<'tcx, 'operand>(tcx: TyCtxt<'tcx>, local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &'operand Operand<'tcx>) -> Option<&'tcx GenericArgs<'tcx>> {
    // 通过Operand获取函数调用的GenericArg
    return get_function_generic_args_from_ty(tcx, &get_operand_ty(tcx, local_decls, operand));
}

fn get_operand_ty<'tcx>(tcx: TyCtxt<'tcx>, local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &Operand<'tcx>) -> Ty<'tcx> {
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
            return ty;
        }
    }
}

fn get_function_generic_args_from_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: &ty::Ty<'tcx>) -> Option<&'tcx GenericArgs<'tcx>> {
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
            trace!("get_function_path_from_ty func_path_with_args: {}", func_path_with_args);
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