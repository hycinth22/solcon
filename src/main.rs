#![feature(rustc_private)]
#![feature(box_patterns)]
#![feature(rustc_attrs)]
#![allow(rustc::untranslatable_diagnostic)]
#![allow(rustc::diagnostic_outside_of_impl)]
#![allow(internal_features)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_mut)]

extern crate tracing; // share from rustc
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

use tracing::{info, trace};
use rustc_driver::Compilation;
use rustc_session::EarlyDiagCtxt;
use rustc_session::config::ErrorOutputType;
use rustc_middle::ty::Instance;
use rustc_middle::mir::mono::MonoItem;
use std::path::PathBuf;
use std::env;

mod mirpass;
mod utils;


// inspired by lockbud & miri
fn main() {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    utils::jemalloc_magic();

    // Snapshot a copy of the environment before `rustc` starts messing with it.
    // (`install_ice_hook` might change `RUST_BACKTRACE`.)
    let _env_snapshot = std::env::vars_os().collect::<Vec<_>>();

    // Add an ICE hook.
    let using_internal_features = rustc_driver::install_ice_hook("internal complier error", |_| ());
    
    // Install the ctrlc handler that sets `rustc_const_eval::CTRL_C_RECEIVED`
    rustc_driver::install_ctrlc_handler();

    // Initialize early diagnostics context
    let early_dcx = EarlyDiagCtxt::new(ErrorOutputType::default());

    // Check & collect arguments
    let mut args = rustc_driver::args::raw_args(&early_dcx).unwrap_or_else(|_| std::process::exit(rustc_driver::EXIT_FAILURE));
    assert!(!args.is_empty());

    // Initialize loggers.
    let mut logger_inited = false;
    if std::env::var_os("RUSTC_LOG").is_some() {
        rustc_driver::init_logger(&early_dcx, utils::rustc_logger_config()); // similar to rustc_driver::init_rustc_env_logger(&early_dcx), but init_rustc_env_logger use environment variable
        info!("init_logger from RUSTC_LOG");
        logger_inited = true;
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
        let Some(sysroot_path) = utils::find_sysroot() else {
            early_dcx.early_fatal("Could not find sysroot. Specify the RUST_SYSROOT environment variable, \
            or use rustup to set the compiler to use for solcon_instrumenter")
        };
        rustc_command_line_arguments.push(sysroot_path);
    }

    // note must start with lib & end with .rlib(e.g lib*.rlib)
    let solcon_monitor_function_lib_crate_name = "this_is_our_monitor_function";
    // see https://github.com/rust-lang/rust/blob/a71c3ffce9ca505af27f43cd3bad7606a72e3ec8/compiler/rustc_metadata/src/locator.rs#L731
    let Some((solcon_monitor_function_rlib_filepath, solcon_monitor_function_rlib_dirpath)) = utils::find_our_monitor_lib() else {
        early_dcx.early_fatal("solcon monitor function rlib not exist")
    };

   // forcely make our monitor lib become dependency of each crate & linked to each crate
   // see https://github.com/rust-lang/rust/blob/a71c3ffce9ca505af27f43cd3bad7606a72e3ec8/compiler/rustc_metadata/src/locator.rs#L127
   // use --extern to specify direct dependency
   rustc_command_line_arguments.push("--extern".to_owned());
   rustc_command_line_arguments.push(format!("force:{solcon_monitor_function_lib_crate_name}={solcon_monitor_function_rlib_filepath}"));
   // because our monitor lib is dependency of each crate, so downstream crate also transmitively dependent on ur monitor lib and search on directories of -L
   rustc_command_line_arguments.push("-L".to_owned());
   rustc_command_line_arguments.push(format!("dependency={solcon_monitor_function_rlib_dirpath}"));

   
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
        // Tell compiler to generate non optimized mir
        rustc_command_line_arguments.push("-Z".into());
        rustc_command_line_arguments.push("mir-opt-level=0".into());
    }

    if !rustc_command_line_arguments
        .iter()
        .any(|arg| arg.starts_with(&"print_mono_items"))
    {
        // Print mono items
        rustc_command_line_arguments.push("-Z".into());
        rustc_command_line_arguments.push("print_mono_items=lazy".into()); // lazy or eager if needed, see https://github.com/rust-lang/rust/blob/a71c3ffce9ca505af27f43cd3bad7606a72e3ec8/compiler/rustc_monomorphize/src/collector.rs#L1482
    }

    if !rustc_command_line_arguments
        .iter()
        .any(|arg| arg.starts_with(&"unstable-options"))
    {
        // the `-Z unstable-options` flag must also be passed to enable `--extern force:` options
        rustc_command_line_arguments.push("-Z".into());
        rustc_command_line_arguments.push("unstable-options".into());
    }

    let mut callbacks = Callbacks::new(!logger_inited);
    let result = rustc_driver::catch_fatal_errors( || {
        info!("rustc_command_line_arguments {:?}", rustc_command_line_arguments.join(" "));
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


struct Callbacks {
    need_init_logger: bool,
    file_name: String,
    output_directory: PathBuf,
    test_run: bool,
}

impl Callbacks {
    pub fn new(need_init_logger: bool) -> Self {
        Self {
            need_init_logger,
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
        info!("Processing input file: {}", self.file_name);
        for c in &config.crate_check_cfg {
            info!("config.crate_check_cfg {c}");
        }
        if config.opts.test {
            info!("in test only mode");
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
        info!("after_analysis input file: {}", self.file_name);
        let mut global_ctxt = queries.global_ctxt().unwrap();
        global_ctxt.enter(|tcx: rustc_middle::ty::TyCtxt| {
            let dcx = tcx.dcx();
            let opts = &tcx.sess.opts;
            let externs = &opts.externs;
            for (str, extern_entry) in externs.iter() {
                info!("externs {}, force={}", str, extern_entry.force);
            }
            if tcx.sess.dcx().has_errors_or_delayed_bugs().is_some() {
                dcx.fatal("solcon_instrumenter cannot be run on programs that fail compilation");
            }
            dcx.abort_if_errors();

            for item_id in tcx.hir_crate_items(()).free_items() {
                if matches!(tcx.def_kind(item_id.owner_id), rustc_hir::def::DefKind::Fn) {
                    trace!("free_items Function: {}, isroot={}", tcx.def_path_str(item_id.owner_id.def_id), is_root(tcx, item_id.owner_id.def_id));
                }
            }
            for item_id in tcx.hir_crate_items(()).trait_items() {
                if matches!(tcx.def_kind(item_id.owner_id), rustc_hir::def::DefKind::Fn) {
                    trace!("trait_items Function: {}, isroot={}", tcx.def_path_str(item_id.owner_id.def_id), is_root(tcx, item_id.owner_id.def_id));
                }
            }
            for item_id in tcx.hir_crate_items(()).impl_items() {
                if matches!(tcx.def_kind(item_id.owner_id), rustc_hir::def::DefKind::Fn) {
                    trace!("impl_items Function: {}, isroot={}", tcx.def_path_str(item_id.owner_id.def_id), is_root(tcx, item_id.owner_id.def_id));
                }
            }
            for item_id in tcx.hir_crate_items(()).foreign_items() {
                if matches!(tcx.def_kind(item_id.owner_id), rustc_hir::def::DefKind::Fn) {
                    trace!("foreign_items Function: {}, isroot={}", tcx.def_path_str(item_id.owner_id.def_id), is_root(tcx, item_id.owner_id.def_id));
                }
            }

            // init late logger
            if self.need_init_logger {
                let early_dcx = EarlyDiagCtxt::new(tcx.sess.opts.error_format);
                if env::var_os("RUSTC_LOG").is_none() {
                    info!("init late loggers");
                    rustc_driver::init_logger(&early_dcx, utils::rustc_logger_config());
                }
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

            if dcx.has_errors_or_delayed_bugs().is_some() {
                dcx.fatal("solcon_instructmenter cannot be run on programs that fail compilation");
            }
            if tcx.sess.mir_opt_level() > 0 {
                dcx.warn("Notice: You have explicitly enabled MIR optimizations!");
            }
            mirpass::run_our_pass(tcx);
            dcx.abort_if_errors();
            let (items, cgus) = tcx.collect_and_partition_mono_items(());
            info!("cgus.len {}", cgus.len());
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
            dcx.abort_if_errors();
        });
        if self.test_run {
            // We avoid code gen for test cases because LLVM is not used in a thread safe manner.
            Compilation::Stop
        } else {
            Compilation::Continue
        }
    }
}

