use rustc_middle::mir::*;
use rustc_middle::ty::{self, TyCtxt, Instance};
use rustc_middle::mir::Operand::Constant;
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::mir::ConstOperand;

pub fn inject_atomic_prints<'tcx>(tcx: TyCtxt<'tcx>) {
    let cgus = tcx.collect_and_partition_mono_items(()).1;
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
    for instance in instances {
        #[allow(invalid_reference_casting)]
        let mut body = unsafe {
            let immutable_ref = tcx.instance_mir(instance.def);
            let mutable_ptr = immutable_ref as *const Body as *mut Body;
            &mut *mutable_ptr
       };

        // Skip promoted src
        if body.source.promoted.is_some() {
            continue;
        }
        // if tcx.is_foreign_item(def_id) {
        //     // 跳过外部函数
        //     return;
        // }
    // 遍历基本块

        for (block, block_data) in body.basic_blocks_mut().iter_enumerated_mut() {
            if let TerminatorKind::Call { func, args, destination, target: _, unwind: _, call_source: _, fn_span: _} = &mut block_data.terminator_mut().kind {
                let func_path = get_function_name(tcx, &func);
                // println!("found function call: {:?}", func_path);
                if let Some(func_path) = func_path {
                    // 检查是否是对 atomic 库函数的调用
                    if func_path.starts_with("std::sync::atomic::AtomicI32::store") {
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
                        // 把返回值修改成666
                        //let stmts_to_add = vec![
                            // Statement {
                            //     source_info: SourceInfo::outermost(DUMMY_SP),
                            //     kind: StatementKind::Assign(
                            //         Box::new(
                            //             destination.clone(),

                            //         )
                            //     ),
                            // }
                        //];

                        let index = block_data.statements.len(); // 在当前块的末尾插入
                        // block_data.statements.extend(stmts_to_add.clone());
                    }
                }
            }
        }

    }
}

// fn _inject<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx mut Body<'tcx>) {

// }

fn get_function_name<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Option<String> {
    // 通过Operand获取函数调用的名称
    match operand {
        Operand::Constant(box ConstOperand { const_, .. }) => {
            match const_ {
                Const::Ty( ty_const) => {
                    if let ty::FnDef(def_id, _) = ty_const.ty().kind() {
                        let func_path = tcx.def_path_str(def_id);
                        // trace!("Const::Ty {:?}", func_path);
                        return Some(func_path);
                    }
                    println!("get_function_name: Const::Ty failed!!!!!!!!!!!!!!");
                }
                Const::Unevaluated(val, _ty) => {
                    let def_id = val.def;
                    return Some(tcx.def_path_str(def_id).to_string());
                    //let substs = substs;
                    //if let Ok(Some(instance)) = ty::Instance::resolve(tcx, ty::ParamEnv::reveal_all(), def_id, substs) {
                    //    if let ty::InstanceDef::Item(def_id) = instance.def {
                    //        return Some(tcx.def_path_str(def_id).to_string());
                    //    }
                    //}
                }
                Const::Val( _cv, ty) => {
                    if let ty::FnDef(def_id, _) = ty.kind() {
                        let func_path = tcx.def_path_str(def_id);
                        // println!("get_function_name: Const::Val {:?}", func_path);
                        return Some(func_path);
                    }
                    println!("get_function_name: Const::Val failed!!!!!!!!!!!!!!");
                }
            }
        }
        _ => {}
    }
    None
}
