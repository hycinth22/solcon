#![feature(rustc_private)]
#![feature(box_patterns)]
#![feature(rustc_attrs)]

extern crate tracing;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_log;
extern crate rustc_type_ir;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use rustc_driver::Compilation;
use rustc_session::EarlyDiagCtxt;
use rustc_session::config::ErrorOutputType;
use std::path::PathBuf;
use std::env;
use std::str::FromStr;
use log::debug;

mod mirpass;

// inspired by lockbud & miri
fn main() {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    jemalloc_magic();

    let early_dcx = EarlyDiagCtxt::new(ErrorOutputType::default());

    // Snapshot a copy of the environment before `rustc` starts messing with it.
    // (`install_ice_hook` might change `RUST_BACKTRACE`.)
    let _env_snapshot = std::env::vars_os().collect::<Vec<_>>();

    let mut args = rustc_driver::args::raw_args(&early_dcx).unwrap_or_else(|_| std::process::exit(rustc_driver::EXIT_FAILURE));
    assert!(!args.is_empty());
    
    // Install the ctrlc handler that sets `rustc_const_eval::CTRL_C_RECEIVED`
    rustc_driver::install_ctrlc_handler();

    // Add an ICE hook.
    let using_internal_features = rustc_driver::install_ice_hook("internal complier error", |_| ());

    // Initialize loggers.
    // let early_dcx = rustc_session::EarlyDiagCtxt::new(ErrorOutputType::default());
    // if std::env::var("RUSTC_LOG").is_ok() {
    //     rustc_driver::init_rustc_env_logger(&early_dcx);
    // }
    if std::env::var_os("RUSTC_LOG").is_some() {
        rustc_driver::init_logger(&early_dcx, rustc_logger_config());
        println!("init_logger from RUSTC_LOG");
    }

    // Setting RUSTC_WRAPPER causes Cargo to pass 'rustc' as the first argument.
    // We're invoking the compiler programmatically, so we remove it if present.
    if args.len() > 1 && std::path::Path::new(&args[1]).file_stem() == Some("rustc".as_ref()) {
        args.remove(1);
    }

    let mut rustc_command_line_arguments: Vec<String> = args[1..].into();
    // Add back the binary name
    rustc_command_line_arguments.insert(0, args[0].clone());

    let sysroot: String = "--sysroot".into();
    if !rustc_command_line_arguments
        .iter()
        .any(|arg| arg == &sysroot)
    {
        // Tell compiler where to find the std library and so on.
        // The compiler relies on the standard rustc driver to tell it, so we have to do likewise.
        rustc_command_line_arguments.push(sysroot);
        rustc_command_line_arguments.push(find_sysroot());
    }

    let always_encode_mir: String = "always-encode-mir".into();
    if !rustc_command_line_arguments
        .iter()
        .any(|arg| arg == &always_encode_mir)
    {
        // Tell compiler to emit MIR into crate for every function with a body.
        rustc_command_line_arguments.push("-Z".into());
        rustc_command_line_arguments.push(always_encode_mir);
    }

    if !rustc_command_line_arguments
        .iter()
        .any(|arg| arg.starts_with(&"mir-opt-level"))
    {
        // Tell compiler to generate non optimized builds
        rustc_command_line_arguments.push("-Z".into());
        rustc_command_line_arguments.push("mir-opt-level=0".into());
    }

    if !rustc_command_line_arguments
        .iter()
        .any(|arg| arg.starts_with(&"print_mono_items"))
    {
        // see https://github.com/rust-lang/rust/blob/a71c3ffce9ca505af27f43cd3bad7606a72e3ec8/compiler/rustc_monomorphize/src/collector.rs#L1482
        rustc_command_line_arguments.push("-Z".into());
        rustc_command_line_arguments.push("print_mono_items=lazy".into());
    }

    // let link_dead_code: String = "link-dead-code".into();
    // if !rustc_command_line_arguments
    //     .iter()
    //     .any(|arg| arg.ends_with(&link_dead_code))
    // {
    //     // Tell compiler to link dead code
    //     rustc_command_line_arguments.push("-C".into());
    //     rustc_command_line_arguments.push(link_dead_code);
    // }

    let mut callbacks = Callbacks::new();
    let result = rustc_driver::catch_fatal_errors( || {
        debug!("rustc_command_line_arguments {:?}", rustc_command_line_arguments);
        let compiler = rustc_driver::RunCompiler::new(&rustc_command_line_arguments, &mut callbacks);
        compiler
        .set_using_internal_features(using_internal_features)
        .run()
    });

    let exit_code = match result {
        Ok(_) => rustc_driver::EXIT_SUCCESS,
        Err(_) => rustc_driver::EXIT_FAILURE,
    };
    std::process::exit(exit_code);
}

fn find_sysroot() -> String {
    if let Some(sysroot) = option_env!("RUST_SYSROOT") {
        return sysroot.to_owned();
    }
    let home = option_env!("RUSTUP_HOME");
    let toolchain = option_env!("RUSTUP_TOOLCHAIN");
    if let (Some(home), Some(toolchain)) = (home, toolchain) {
        return format!("{}/toolchains/{}", home, toolchain);
    }
    let out = std::process::Command::new("rustc").arg("--print=sysroot")
    .current_dir(".").output();
    if let Ok(out) = out {
        if out.status.success() {
            let sysroot = std::str::from_utf8(&out.stdout).unwrap().trim();
            return sysroot.to_owned();
        }
    }
    panic!("Could not find sysroot. Specify the RUST_SYSROOT environment variable, \
    or use rustup to set the compiler to use for solcon",)
}


fn rustc_logger_config() -> rustc_log::LoggerConfig {
    // Start with the usual env vars.
    let mut cfg = rustc_log::LoggerConfig::from_env("RUSTC_LOG");

    // Overwrite if MIRI_LOG is set.
    if let Ok(var) = env::var("SOLCON_LOG") {
        // MIRI_LOG serves as default for RUSTC_LOG, if that is not set.
        if matches!(cfg.filter, Err(env::VarError::NotPresent)) {
            // We try to be a bit clever here: if `SOLCON_LOG` is just a single level
            // used for everything, we only apply it to the parts of rustc that are
            // CTFE-related. Otherwise, we use it verbatim for `RUSTC_LOG`.
            // This way, if you set `MIRI_LOG=trace`, you get only the right parts of
            // rustc traced, but you can also do `MIRI_LOG=miri=trace,rustc_const_eval::interpret=debug`.
            if tracing::Level::from_str(&var).is_ok() {
                cfg.filter = Ok(format!(
                    "rustc_middle::mir::interpret={var},rustc_const_eval::interpret={var},miri={var}" // todo
                ));
            } else {
                cfg.filter = Ok(var);
            }
        }
    }

    cfg
}

struct Callbacks {
    file_name: String,
    output_directory: PathBuf,
    test_run: bool,
}

impl Callbacks {
    pub fn new() -> Self {
        Self {
            file_name: String::new(),
            output_directory: PathBuf::default(),
            test_run: false,
        }
    }
}

fn is_root<'tcx>(tcx: rustc_middle::ty::TyCtxt<'tcx>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    !tcx.generics_of(def_id).requires_monomorphization(tcx)
}

impl rustc_driver::Callbacks for Callbacks {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        self.file_name = config.input.source_name().prefer_remapped_unconditionaly().to_string();
        debug!("Processing input file: {}", self.file_name);
        if config.opts.test {
            debug!("in test only mode");
            self.test_run = true;
        }
        match &config.output_dir {
            None => {
                self.output_directory = std::env::temp_dir();
                self.output_directory.pop();
            }
            Some(path_buf) => self.output_directory.push(path_buf.as_path()),
        }
    }
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        queries: &'tcx rustc_interface::Queries<'tcx>,
    ) -> rustc_driver::Compilation {
        if self
            .output_directory
            .to_str()
            .expect("valid string")
            .contains("/build/")
        {
            // No need to analyze a build script, but do generate code.
            return Compilation::Continue;
        }
        let mut global_ctxt = queries.global_ctxt().unwrap();
        global_ctxt.enter(|tcx: rustc_middle::ty::TyCtxt| {
            for item_id in tcx.hir_crate_items(()).free_items() {
                if matches!(tcx.def_kind(item_id.owner_id), rustc_hir::def::DefKind::Fn) {
                    println!("free_items Function: {}, isroot={}", tcx.def_path_str(item_id.owner_id.def_id), is_root(tcx, item_id.owner_id.def_id));
                }
            }
            for item_id in tcx.hir_crate_items(()).trait_items() {
                if matches!(tcx.def_kind(item_id.owner_id), rustc_hir::def::DefKind::Fn) {
                    println!("trait_items Function: {}, isroot={}", tcx.def_path_str(item_id.owner_id.def_id), is_root(tcx, item_id.owner_id.def_id));
                }
            }
            for item_id in tcx.hir_crate_items(()).impl_items() {
                if matches!(tcx.def_kind(item_id.owner_id), rustc_hir::def::DefKind::Fn) {
                    println!("impl_items Function: {}, isroot={}", tcx.def_path_str(item_id.owner_id.def_id), is_root(tcx, item_id.owner_id.def_id));
                }
            }
            for item_id in tcx.hir_crate_items(()).foreign_items() {
                if matches!(tcx.def_kind(item_id.owner_id), rustc_hir::def::DefKind::Fn) {
                    println!("foreign_items Function: {}, isroot={}", tcx.def_path_str(item_id.owner_id.def_id), is_root(tcx, item_id.owner_id.def_id));
                }
            }
            // init_late_loggers
            let early_dcx = EarlyDiagCtxt::new(tcx.sess.opts.error_format);
            if env::var_os("RUSTC_LOG").is_none() {
                println!("init_logger");
                rustc_driver::init_logger(&early_dcx, rustc_logger_config());
            }

            // If `SOLCON_BACKTRACE` is set and `RUSTC_CTFE_BACKTRACE` is not, set `RUSTC_CTFE_BACKTRACE`.
            // Do this late, so we ideally only apply this to SOLCON's errors.
            if let Some(val) = env::var_os("SOLCON_BACKTRACE") {
                let ctfe_backtrace = match &*val.to_string_lossy() {
                    "immediate" => rustc_session::CtfeBacktrace::Immediate,
                    "0" => rustc_session::CtfeBacktrace::Disabled,
                    _ => rustc_session::CtfeBacktrace::Capture,
                };
                *tcx.sess.ctfe_backtrace.borrow_mut() = ctfe_backtrace;
            }

            if tcx.sess.dcx().has_errors_or_delayed_bugs().is_some() {
                tcx.dcx().fatal("solcon_instructmenter cannot be run on programs that fail compilation");
            }
            if tcx.sess.mir_opt_level() > 0 {
                tcx.dcx().warn("Notice: You have explicitly enabled MIR optimizations!");
            }


            mirpass::run_our_pass(tcx);
            //tcx.collect_and_partition_mono_items(());
            //tcx.hir_crate(());
            tcx.dcx().abort_if_errors();
        });
        if self.test_run {
            // We avoid code gen for test cases because LLVM is not used in a thread safe manner.
            Compilation::Stop
        } else {
            Compilation::Continue
        }
    }
}


#[cfg(any(target_os = "linux", target_os = "macos"))]
fn jemalloc_magic() {
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