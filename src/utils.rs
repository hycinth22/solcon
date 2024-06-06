use std::env;
use std::path::Path;
use std::str::FromStr;
use rustc_middle::ty::{self, GenericArgs, Ty, TyCtxt};
use rustc_middle::mir::*;
use rustc_span::DUMMY_SP;
use rustc_span::def_id::DefId;
use rustc_hir::definitions::DefPath;
use tracing::{trace, info};
use crate::config;

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
        return Some(format!("{home}/toolchains/{toolchain}"));
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

pub fn find_our_monitor_lib() -> Option<(String, String, String)>  {
    if let Ok(lib_path) = env::var("SOLCON_MONITOR_LIB_PATH") {
        info!("find env SOLCON_MONITOR_LIB_PATH: {}", lib_path);
        let dir_path = Path::new(&lib_path).parent()?;
        let lib_deps_path: String = dir_path.join("deps").to_str()?.to_owned();
        return Some((lib_path.to_owned(), dir_path.to_str()?.to_owned(), lib_deps_path));
    }
    let current_dir = env::current_dir().ok()?;
    let lib_file_path = current_dir.join(config::MONITORS_LIB_DEFAULT_FILEPATH);
    if lib_file_path.exists() {
        let dir_path = Path::new(&lib_file_path).parent()?;
        let lib_deps_path: String = dir_path.join("deps").to_str()?.to_owned();
        return Some((String::from(lib_file_path.to_str()?), dir_path.to_str()?.to_owned(), lib_deps_path));
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
            // instrument-related. Otherwise, we use it verbatim for `RUSTC_LOG`.
            // This way, if you set `SOLCON_LOG=trace`, you get only the right parts of
            // rustc traced, but you can also do `SOLCON_LOG=miri=trace,rustc_const_eval::interpret=debug`.
            if tracing::Level::from_str(&var).is_ok() {
                cfg.filter = Ok(format!(
                    "rustc_metadata={var},rustc_resolve={var},rustc_interface={var}"
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
    return get_function_path_from_ty(tcx, &operand.ty(local_decls, tcx));
}

pub fn get_function_path_str<'tcx, 'operand>(tcx: TyCtxt<'tcx>, local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &'operand Operand<'tcx>) -> Option<String> {
    // 通过Operand获取函数调用的名称
    return get_function_path_str_from_ty(tcx, &operand.ty(local_decls, tcx));
}

pub fn get_function_generic_args<'tcx, 'operand>(tcx: TyCtxt<'tcx>, local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &'operand Operand<'tcx>) -> Option<&'tcx GenericArgs<'tcx>> {
    // 通过Operand获取函数调用的GenericArg
    return get_function_generic_args_from_ty(&operand.ty(local_decls, tcx));
}

#[deprecated(since = "0.2.0", note = "Use operand.ty(local_decls, tcx) instead")]
pub fn get_operand_ty<'tcx>(tcx: TyCtxt<'tcx>, local_decls: &rustc_index::IndexVec<Local, LocalDecl<'tcx>>, operand: &Operand<'tcx>) -> Ty<'tcx> {
    operand.ty(local_decls, tcx)
}

pub fn get_function_generic_args_from_ty<'tcx>(ty: &ty::Ty<'tcx>) -> Option<&'tcx GenericArgs<'tcx>> {
    let ty_kind: &rustc_type_ir::TyKind<TyCtxt> = ty.kind();
    match ty_kind {
        ty::TyKind::FnDef(_def_id, args) // // The anonymous type of a function declaration/definition
        | ty::Closure(_def_id, args) // // The anonymous type of a closure. Used to represent the type of |a| a.
        | ty::TyKind::CoroutineClosure(_def_id, args)  // The anonymous type of a closure. Used to represent the type of async |a| a.
        | ty::TyKind::Coroutine(_def_id, args) // The anonymous type of a coroutine. Used to represent the type of |a| yield a.
        => {
            return Some(&args);
        }
        ty::FnPtr(_) => {
            info!("get_function_generic_args_from_ty failed: we cannot infer FnPtr point to what");   
        },
        // A trait object. Written as dyn for<'b> Trait<'b, Assoc = u32> + Send + 'a.
        ty::TyKind::Dynamic(_, _, _) => unimplemented!(),
        // A placeholder for a type which could not be computed; this is propagated to avoid useless error messages.
        ty::TyKind::Error(_) => {
            return None;
        },
        // the following all types looks unlikely because uncallable, but remains their different branchs for debug
        ty::TyKind::Bool => unreachable!(),
        ty::TyKind::Char => unreachable!(),
        ty::TyKind::Int(_) => unreachable!(),
        ty::TyKind::Uint(_) => unreachable!(),
        ty::TyKind::Float(_) => unreachable!(),
        // Algebraic data types (ADT). For example: structures, enumerations and unions.
        ty::TyKind::Adt(_, _) => unreachable!(),
        // An unsized FFI type that is opaque to Rust. Written as extern type T.
        ty::TyKind::Foreign(_) => unreachable!(),
        // The pointee of a string slice. Written as str.
        ty::TyKind::Str => unreachable!(),
        // An array with the given length. Written as [T; N].
        ty::TyKind::Array(_, _) => unreachable!(),
        // A pattern newtype.  Only supports integer range patterns for now.
        ty::TyKind::Pat(_, _) => unreachable!(),
        // The pointee of an array slice. Written as [T]
        ty::TyKind::Slice(_) => unreachable!(),
        // A raw pointer. Written as *mut T or *const T
        ty::TyKind::RawPtr(_, _) => unreachable!(),
        // A reference; a pointer with an associated lifetime. Written as &'a mut T or &'a T.
        ty::TyKind::Ref(_, _, _) => unreachable!(),
        // A type representing the types stored inside a coroutine. This should only appear as part of the CoroutineArgs.
        ty::TyKind::CoroutineWitness(_, _) => unreachable!(),
        // The never type !.
        ty::TyKind::Never => unreachable!(),
        // A tuple type. For example, (i32, bool).
        ty::TyKind::Tuple(_) => unreachable!(),
        // A projection, opaque type, weak type alias, or inherent associated type.
        ty::TyKind::Alias(_, _) => unreachable!(),
        // A type parameter; for example, T in fn f<T>(x: T) {}.
        ty::TyKind::Param(_) => unreachable!(),
        // Bound type variable, used to represent the 'a in for<'a> fn(&'a ()).
        ty::TyKind::Bound(_, _) => unreachable!(),
        // A placeholder type, used during higher ranked subtyping to instantiate bound variables.
        ty::TyKind::Placeholder(_) => unreachable!(),
        // A type variable used during type checking.
        ty::TyKind::Infer(_) => unreachable!(),
    }
    info!("get_function_generic_args_from_ty: failed!!!!!!!!!!!!!!");
    None
}

pub fn get_function_path_str_from_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: &ty::Ty<'tcx>) -> Option<String> {
    let ty_kind = ty.kind();
    match ty_kind {
        ty::TyKind::FnDef(def_id, _args) // // The anonymous type of a function declaration/definition
        | ty::Closure(def_id, _args) // // The anonymous type of a closure. Used to represent the type of |a| a.
        | ty::TyKind::CoroutineClosure(def_id, _args)  // The anonymous type of a closure. Used to represent the type of async |a| a.
        | ty::TyKind::Coroutine(def_id, _args) // The anonymous type of a coroutine. Used to represent the type of |a| yield a.
        => {
            //let func_def_path_str_with_args = tcx.def_path_str_with_args(def_id, _args);
            //dbg!(_args);
            //debug!("get_function_path_str_from_ty func_def_path_str_with_args: {}", func_def_path_str_with_args);
            let func_def_path_str = tcx.def_path_str(*def_id);
            return Some(func_def_path_str);
        }
        ty::FnPtr(_) => {
            info!("get_function_path_str_from_ty failed: we cannot infer FnPtr point to what");   
        },
        // A trait object. Written as dyn for<'b> Trait<'b, Assoc = u32> + Send + 'a.
        ty::TyKind::Dynamic(_, _, _) => unimplemented!(),
        // A placeholder for a type which could not be computed; this is propagated to avoid useless error messages.
        ty::TyKind::Error(_) => {
            return None;
        },
        // the following all types looks unlikely because uncallable, but remains their different branchs for debug
        ty::TyKind::Bool => unreachable!(),
        ty::TyKind::Char => unreachable!(),
        ty::TyKind::Int(_) => unreachable!(),
        ty::TyKind::Uint(_) => unreachable!(),
        ty::TyKind::Float(_) => unreachable!(),
        // Algebraic data types (ADT). For example: structures, enumerations and unions.
        ty::TyKind::Adt(_, _) => unreachable!(),
        // An unsized FFI type that is opaque to Rust. Written as extern type T.
        ty::TyKind::Foreign(_) => unreachable!(),
        // The pointee of a string slice. Written as str.
        ty::TyKind::Str => unreachable!(),
        // An array with the given length. Written as [T; N].
        ty::TyKind::Array(_, _) => unreachable!(),
        // A pattern newtype.  Only supports integer range patterns for now.
        ty::TyKind::Pat(_, _) => unreachable!(),
        // The pointee of an array slice. Written as [T]
        ty::TyKind::Slice(_) => unreachable!(),
        // A raw pointer. Written as *mut T or *const T
        ty::TyKind::RawPtr(_, _) => unreachable!(),
        // A reference; a pointer with an associated lifetime. Written as &'a mut T or &'a T.
        ty::TyKind::Ref(_, _, _) => unreachable!(),
        // A type representing the types stored inside a coroutine. This should only appear as part of the CoroutineArgs.
        ty::TyKind::CoroutineWitness(_, _) => unreachable!(),
        // The never type !.
        ty::TyKind::Never => unreachable!(),
        // A tuple type. For example, (i32, bool).
        ty::TyKind::Tuple(_) => unreachable!(),
        // A projection, opaque type, weak type alias, or inherent associated type.
        ty::TyKind::Alias(_, _) => unreachable!(),
        // A type parameter; for example, T in fn f<T>(x: T) {}.
        ty::TyKind::Param(_) => unreachable!(),
        // Bound type variable, used to represent the 'a in for<'a> fn(&'a ()).
        ty::TyKind::Bound(_, _) => unreachable!(),
        // A placeholder type, used during higher ranked subtyping to instantiate bound variables.
        ty::TyKind::Placeholder(_) => unreachable!(),
        // A type variable used during type checking.
        ty::TyKind::Infer(_) => unreachable!(),
    }
    info!("get_function_path_str_from_ty: failed!!!!!!!!!!!!!!");
    None
}


pub fn get_function_path_from_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: &ty::Ty<'tcx>) -> Option<DefPath> {
    let ty_kind = ty.kind();
    match ty_kind {
        ty::TyKind::FnDef(def_id, _args) // // The anonymous type of a function declaration/definition
        | ty::Closure(def_id, _args) // // The anonymous type of a closure. Used to represent the type of |a| a.
        | ty::TyKind::CoroutineClosure(def_id, _args)  // The anonymous type of a closure. Used to represent the type of async |a| a.
        | ty::TyKind::Coroutine(def_id, _args) // The anonymous type of a coroutine. Used to represent the type of |a| yield a.
        => {
            //let func_def_path_str_with_args = tcx.def_path_str_with_args(def_id, _args);
            //dbg!(_args);
            //debug!("get_function_path_str_from_ty func_def_path_str_with_args: {}", func_def_path_str_with_args);
            let def_path = tcx.def_path(def_id.clone());
            return Some(def_path);
        }
        ty::FnPtr(_) => {
            info!("get_function_path_from_ty failed: we cannot infer FnPtr point to what");   
        },
        // A trait object. Written as dyn for<'b> Trait<'b, Assoc = u32> + Send + 'a.
        ty::TyKind::Dynamic(_, _, _) => unimplemented!(),
        // A placeholder for a type which could not be computed; this is propagated to avoid useless error messages.
        ty::TyKind::Error(_) => {
            return None;
        },
        // the following all types looks unlikely because uncallable, but remains their different branchs for debug
        ty::TyKind::Bool => unreachable!(),
        ty::TyKind::Char => unreachable!(),
        ty::TyKind::Int(_) => unreachable!(),
        ty::TyKind::Uint(_) => unreachable!(),
        ty::TyKind::Float(_) => unreachable!(),
        // Algebraic data types (ADT). For example: structures, enumerations and unions.
        ty::TyKind::Adt(_, _) => unreachable!(),
        // An unsized FFI type that is opaque to Rust. Written as extern type T.
        ty::TyKind::Foreign(_) => unreachable!(),
        // The pointee of a string slice. Written as str.
        ty::TyKind::Str => unreachable!(),
        // An array with the given length. Written as [T; N].
        ty::TyKind::Array(_, _) => unreachable!(),
        // A pattern newtype.  Only supports integer range patterns for now.
        ty::TyKind::Pat(_, _) => unreachable!(),
        // The pointee of an array slice. Written as [T]
        ty::TyKind::Slice(_) => unreachable!(),
        // A raw pointer. Written as *mut T or *const T
        ty::TyKind::RawPtr(_, _) => unreachable!(),
        // A reference; a pointer with an associated lifetime. Written as &'a mut T or &'a T.
        ty::TyKind::Ref(_, _, _) => unreachable!(),
        // A type representing the types stored inside a coroutine. This should only appear as part of the CoroutineArgs.
        ty::TyKind::CoroutineWitness(_, _) => unreachable!(),
        // The never type !.
        ty::TyKind::Never => unreachable!(),
        // A tuple type. For example, (i32, bool).
        ty::TyKind::Tuple(_) => unreachable!(),
        // A projection, opaque type, weak type alias, or inherent associated type.
        ty::TyKind::Alias(_, _) => unreachable!(),
        // A type parameter; for example, T in fn f<T>(x: T) {}.
        ty::TyKind::Param(_) => unreachable!(),
        // Bound type variable, used to represent the 'a in for<'a> fn(&'a ()).
        ty::TyKind::Bound(_, _) => unreachable!(),
        // A placeholder type, used during higher ranked subtyping to instantiate bound variables.
        ty::TyKind::Placeholder(_) => unreachable!(),
        // A type variable used during type checking.
        ty::TyKind::Infer(_) => unreachable!(),
    }
    info!("get_function_path_from_ty: failed!!!!!!!!!!!!!!");
    None
}

pub fn alloc_unit_local<'tcx>(tcx: TyCtxt<'tcx>, local_decls: &mut rustc_index::IndexVec<Local, LocalDecl<'tcx>>) -> Local {
    let local_decl = LocalDecl::new(tcx.types.unit, DUMMY_SP);
    let new_local= local_decls.push(local_decl);
    return new_local
}

pub fn instantiate_our_func<'tcx>(tcx: TyCtxt<'tcx>, our_func_def_id: DefId, generic_args: &'tcx GenericArgs<'tcx>, fn_span: rustc_span::Span) -> Operand<'tcx> {
    let is_generic_func = tcx.generics_of(our_func_def_id).own_requires_monomorphization(); // generics.own_params.is_empty()
    if is_generic_func {
        Operand::function_handle(tcx, our_func_def_id, generic_args, fn_span.clone())
    } else {
        Operand::function_handle(tcx, our_func_def_id, [], fn_span.clone())
    }
}

pub fn is_fn_like_def(tcx: TyCtxt<'_>, def_id: &DefId) -> bool {
    use rustc_hir::def::DefKind::*;
    use rustc_hir::def::CtorKind;
    let def_kind = tcx.def_kind(def_id);
    match def_kind {
        Fn | AssocFn | Closure => true,
        // Refers to the struct or enum variant’s constructor.
        Ctor(_ctor_of, ctor_kind) => {
            match ctor_kind{
                CtorKind::Fn => false, // Constructor function automatically created by a tuple struct/variant.
                CtorKind::Const => false, // Constructor constant automatically created by a unit struct/variant.
            }
        }
        Mod  | Struct| Union | Enum | Variant | Trait 
        | TyAlias // Type alias: type Foo = Bar;
        | ForeignTy // Type from an extern block.
        | TraitAlias // Trait alias: trait IntIterator = Iterator<Item = i32>;
        | AssocTy // Associated type: trait MyTrait { type Assoc; }
        | TyParam // Type parameter: the T in struct Vec<T> { ... }
        | Const 
        | ConstParam // Constant generic parameter: struct Foo<const N: usize> { ... }
        | Static{..} 
        | AssocConst // Associated constant: trait MyTrait { const ASSOC: usize; }
        | Macro(..) | ExternCrate | Use
        | ForeignMod 
        | AnonConst // Anonymous constant, e.g. the 1 + 2 in [u8; 1 + 2]
        | InlineConst // An inline constant, e.g. const { 1 + 2 }
        | OpaqueTy // Opaque type, aka impl Trait.
        | Field // A field in a struct, enum or union.
        | LifetimeParam // Lifetime parameter: the 'a in struct Foo<'a> { ... }
        | GlobalAsm // A use of global_asm!.
        | Impl{..} => false
    }
}

pub fn span_to_string(tcx: TyCtxt<'_>, span: rustc_span::Span) -> String {
    let source_map = tcx.sess.source_map();
    let str = source_map.span_to_string(span, rustc_span::FileNameDisplayPreference::Remapped);
    trace!("span_to_string: {str}");
    str
}
