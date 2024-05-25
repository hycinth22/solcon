use std::env;
use std::path::Path;
use std::str::FromStr;
use rustc_middle::ty::{self, GenericArgs, Instance, Ty, TyCtxt};
use rustc_middle::mir::*;
use rustc_span::DUMMY_SP;
use rustc_span::def_id::{CrateNum, DefId};
use rustc_hir::definitions::DefPath;
use tracing::{trace, info};

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn jemalloc_magic() {
    // These magic runes are copied from
    // <https://github.com/rust-lang/rust/blob/e89bd9428f621545c979c0ec686addc6563a394e/compiler/rustc/src/main.rs#L39>.
    // See there for further comments.
    use std::os::raw::{c_int, c_void};

    #[used]
    static _F1: unsafe extern "C" fn(usize, usize) -> *mut c_void = jemalloc_sys::calloc;
    #[used]
    static _F2: unsafe extern "C" fn(*mut *mut c_void, usize, usize) -> c_int =
        jemalloc_sys::posix_memalign;
    #[used]
    static _F3: unsafe extern "C" fn(usize, usize) -> *mut c_void = jemalloc_sys::aligned_alloc;
    #[used]
    static _F4: unsafe extern "C" fn(usize) -> *mut c_void = jemalloc_sys::malloc;
    #[used]
    static _F5: unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void = jemalloc_sys::realloc;
    #[used]
    static _F6: unsafe extern "C" fn(*mut c_void) = jemalloc_sys::free;

    // On OSX, jemalloc doesn't directly override malloc/free, but instead
    // registers itself with the allocator's zone APIs in a ctor. However,
    // the linker doesn't seem to consider ctors as "used" when statically
    // linking, so we need to explicitly depend on the function.
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn _rjem_je_zone_register();
        }

        #[used]
        static _F7: unsafe extern "C" fn() = _rjem_je_zone_register;
    }
}

pub fn find_sysroot() -> Option<String> {
    if let Some(sysroot) = option_env!("RUST_SYSROOT") { // notice: complied-time env value
        return Some(sysroot.to_owned());
    }
    let home = option_env!("RUSTUP_HOME");
    let toolchain = option_env!("RUSTUP_TOOLCHAIN");
    if let (Some(home), Some(toolchain)) = (home, toolchain) {
        return Some(format!("{}/toolchains/{}", home, toolchain));
    }
    let out = std::process::Command::new("rustc").arg("--print=sysroot")
    .current_dir(".").output();
    if let Ok(out) = out {
        if out.status.success() {
            let sysroot = std::str::from_utf8(&out.stdout).unwrap().trim();
            return Some(sysroot.to_owned());
        }
    }
    None
}

fn get_parent_path(path: &str) -> Option<String> {
    let parent_path = Path::new(path).parent()?.to_str()?;
    Some(parent_path.into())
}

pub fn find_our_monitor_lib() -> Option<(String, String)>  {
    if let Ok(lib_path) = env::var("SOLCON_MONITOR_LIB_PATH") {
        info!("find env SOLCON_MONITOR_LIB_PATH: {}", lib_path);
        let dir_path = get_parent_path(lib_path.as_str())?;
        return Some((lib_path, dir_path));
    }
    let current_dir = env::current_dir().ok()?;
    const LIB_FILE_NAME :&str = "this_is_our_monitor_function/target/debug/libthis_is_our_monitor_function.rlib";
    let lib_file_path = current_dir.join(LIB_FILE_NAME);
    if lib_file_path.exists() {
        return Some((String::from(lib_file_path.to_str()?), get_parent_path(lib_file_path.to_str()?)?));
    }
    info!("fail to find our monitor lib {}", String::from(lib_file_path.to_str()?));
    None
}

pub fn rustc_logger_config() -> rustc_log::LoggerConfig {
    // Start with the usual env vars.
    let mut cfg = rustc_log::LoggerConfig::from_env("RUSTC_LOG");

    // Overwrite if SOLCON_LOG is set.
    if let Ok(var) = env::var("SOLCON_LOG") {
        // SOLCON_LOG serves as default for RUSTC_LOG, if that is not set.
        if matches!(cfg.filter, Err(env::VarError::NotPresent)) {
            // We try to be a bit clever here: if `SOLCON_LOG` is just a single level
            // used for everything, we only apply it to the parts of rustc that are
            // CTFE-related. Otherwise, we use it verbatim for `RUSTC_LOG`.
            // This way, if you set `SOLCON_LOG=trace`, you get only the right parts of
            // rustc traced, but you can also do `SOLCON_LOG=miri=trace,rustc_const_eval::interpret=debug`.
            if tracing::Level::from_str(&var).is_ok() {
                cfg.filter = Ok(format!(
                    "rustc_middle::mir::interpret={var},rustc_const_eval::interpret={var},solcon_instrumenter={var}" // todo
                ));
            } else {
                cfg.filter = Ok(var);
            }
        }
    }

    cfg
}

pub fn file_exist(file_path: &str) -> bool {
    match std::fs::metadata(file_path) {
        Ok(metadata) => {
            // 检查元数据中的文件类型是否是一个文件
            metadata.is_file()
        },
        Err(_) => {
            false
        }
    }
}

pub fn is_crate_def_id(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    let def_path = tcx.def_path(def_id);
    // 获取 DefPath 的第一个元素
    if let Some(first_elem) = def_path.data.iter().next() {
        // 检查第一个元素是否是 CrateRoot
        return matches!(first_elem.data, rustc_hir::definitions::DefPathData::CrateRoot);
    }
    false
}

pub fn get_function_path<'tcx, 'operand>(tcx: TyCtxt<'tcx>, local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &'operand Operand<'tcx>) -> Option<DefPath> {
    // 通过Operand获取函数调用的名称
    return get_function_path_from_ty(tcx, &get_operand_ty( local_decls, operand));
}

pub fn get_function_path_str<'tcx, 'operand>(tcx: TyCtxt<'tcx>, local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &'operand Operand<'tcx>) -> Option<String> {
    // 通过Operand获取函数调用的名称
    return get_function_path_str_from_ty(tcx, &get_operand_ty( local_decls, operand));
}

pub fn get_function_generic_args<'tcx, 'operand>(local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &'operand Operand<'tcx>) -> Option<&'tcx GenericArgs<'tcx>> {
    // 通过Operand获取函数调用的GenericArg
    return get_function_generic_args_from_ty(&get_operand_ty(local_decls, operand));
}

pub fn get_operand_ty<'tcx>(local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &Operand<'tcx>) -> Ty<'tcx> {
    match operand {
        Operand::Constant(box ConstOperand { const_, .. }) => {
            match const_ {
                Const::Ty( ty_const) => {
                    //trace!("ty!!");
                    return ty_const.ty();
                }
                Const::Unevaluated(_val, ty) => {
                    //trace!("Unevaluated!!");
                    return ty.clone();
                }
                Const::Val( _val, ty) => {
                    //dbg!(_val);
                    //trace!("Val!!");
                    return ty.clone();
                }
            }
        }
        Operand::Copy(place) | Operand::Move(place) => {
            let ty = local_decls[place.local].ty;
            // trace!("Copy | Move !!");
            return ty;
        }
    }
}

pub fn get_function_generic_args_from_ty<'tcx>(ty: &ty::Ty<'tcx>) -> Option<&'tcx GenericArgs<'tcx>> {
    let ty_kind: &rustc_type_ir::TyKind<TyCtxt> = ty.kind();
    match ty_kind {
        ty::TyKind::FnDef(_def_id, args) | ty::Closure(_def_id, args) => {
            return Some(&args);
        }
        ty::FnPtr(_) => {
            info!("get_function_args_from_ty: FnPtr failed!!!!!!!!!!!!!!");   
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
    info!("get_function_path_from_ty: failed!!!!!!!!!!!!!!");
    None
}

pub fn get_function_path_str_from_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: &ty::Ty<'tcx>) -> Option<String> {
    let ty_kind = ty.kind();
    match ty_kind {
        ty::TyKind::FnDef(def_id, _args) | ty::Closure(def_id, _args) => {
            //let func_def_path_str_with_args = tcx.def_path_str_with_args(def_id, _args);
            //dbg!(_args);
            //debug!("get_function_path_str_from_ty func_def_path_str_with_args: {}", func_def_path_str_with_args);
            let func_def_path_str = tcx.def_path_str(*def_id);
            return Some(func_def_path_str);
        }
        ty::FnPtr(_) => {
            info!("get_function_path_str_from_ty: FnPtr failed!!!!!!!!!!!!!!");   
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
    info!("get_function_path_str_from_ty: failed!!!!!!!!!!!!!!");
    None
}


pub fn get_function_path_from_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: &ty::Ty<'tcx>) -> Option<DefPath> {
    let ty_kind = ty.kind();
    match ty_kind {
        ty::TyKind::FnDef(def_id, _args) | ty::Closure(def_id, _args) => {
            //let func_def_path_str_with_args = tcx.def_path_str_with_args(def_id, _args);
            //dbg!(_args);
            //debug!("get_function_path_str_from_ty func_def_path_str_with_args: {}", func_def_path_str_with_args);
            let def_path = tcx.def_path(def_id.clone());
            return Some(def_path);
        }
        ty::FnPtr(_) => {
            info!("get_function_path_from_ty: FnPtr failed!!!!!!!!!!!!!!");   
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
    info!("get_function_path_from_ty: failed!!!!!!!!!!!!!!");
    None
}

pub fn alloc_unit_local<'tcx>(tcx: TyCtxt<'tcx>, local_decls: &mut rustc_index::IndexVec<Local, LocalDecl<'tcx>>) -> Local {
    let local_decl = LocalDecl::new(tcx.types.unit, DUMMY_SP);
    let new_local= local_decls.push(local_decl);
    return new_local
}
