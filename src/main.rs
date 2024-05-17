#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::PathBuf;

use rustc_driver::Compilation;
use rustc_session::config::ErrorOutputType;
use rustc_session::EarlyDiagCtxt;
use log::debug;

mod mirpass;

// inspired by lockbud
fn main() {
    // Initialize loggers.
    println!("t1");
    // let handler = EarlyDiagCtxt::new(ErrorOutputType::default());
    // if std::env::var("RUSTC_LOG").is_ok() {
    //     rustc_driver::init_rustc_env_logger(&handler);
    // }
    // if std::env::var("SOLCON_LOG").is_ok() {
    //     let e = env_logger::Env::new()
    //         .filter("SOLCON_LOG")
    //         .write_style("SOLCON_LOG_STYLE");
    //     env_logger::init_from_env(e);
    // }
    println!("t2");
    let mut args = std::env::args().collect::<Vec<_>>();
    assert!(!args.is_empty());

    
    // Setting RUSTC_WRAPPER causes Cargo to pass 'rustc' as the first argument.
    // We're invoking the compiler programmatically, so we remove it if present.
    if args.len() > 1 && std::path::Path::new(&args[1]).file_stem() == Some("rustc".as_ref()) {
        args.remove(1);
    }

    let mut rustc_command_line_arguments: Vec<String> = args[1..].into();
    // rustc_driver::install_ice_hook("ice ice ice baby", |_| ());
    let result = rustc_driver::catch_fatal_errors( || {
        // Add back the binary name
        rustc_command_line_arguments.insert(0, args[0].clone());

        let sysroot: String = "--sysroot".into();
        if !rustc_command_line_arguments
            .iter()
            .any(|arg| arg.starts_with(&sysroot))
        {
            // Tell compiler where to find the std library and so on.
            // The compiler relies on the standard rustc driver to tell it, so we have to do likewise.
            rustc_command_line_arguments.push(sysroot);
            rustc_command_line_arguments.push(find_sysroot());
        }

        let always_encode_mir: String = "always-encode-mir".into();
        if !rustc_command_line_arguments
            .iter()
            .any(|arg| arg.ends_with(&always_encode_mir))
        {
            // Tell compiler to emit MIR into crate for every function with a body.
            rustc_command_line_arguments.push("-Z".into());
            rustc_command_line_arguments.push(always_encode_mir);
        }

        let mut callbacks = Callbacks::new();
        debug!(
            "rustc_command_line_arguments {:?}",
            rustc_command_line_arguments
        );
        let compiler = rustc_driver::RunCompiler::new(&rustc_command_line_arguments, &mut callbacks);
        compiler.run()
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

impl rustc_driver::Callbacks for Callbacks {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        self.file_name = config.input.source_name().prefer_remapped_unconditionaly().to_string();
        debug!("Processing input file: {}", self.file_name);
        if config.opts.test {
            debug!("in test only mode");
            self.test_run = true;
            // self.options.test_only = true;
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
        compiler: &rustc_interface::interface::Compiler,
        queries: &'tcx rustc_interface::Queries<'tcx>,
    ) -> rustc_driver::Compilation {
        // compiler.sess.abort_if_errors();
        if self
            .output_directory
            .to_str()
            .expect("valid string")
            .contains("/build/")
        {
            // No need to analyze a build script, but do generate code.
            return Compilation::Continue;
        }
        queries.global_ctxt().unwrap().enter(|tcx| {
            mirpass::inject_atomic_prints(tcx);
        });
        if self.test_run {
            // We avoid code gen for test cases because LLVM is not used in a thread safe manner.
            Compilation::Stop
        } else {
            Compilation::Continue
        }
    }
}