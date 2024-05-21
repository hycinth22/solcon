use std::env;
use std::fs;
use std::path::Path;

const WINDOWS_DEPENDENTS : [&str; 2] = [
    "rustc_driver-5a9cbca8b3eb4ddf.dll",
    "std-ac24efe4baa6f4b5.dll",
];

const WINDOWS_DEPENDENTS_DEBUG : [&str; 2] = [
    "rustc_driver-5a9cbca8b3eb4ddf.pdb",
    "std-ac24efe4baa6f4b5.pdb",
];


fn main() {
    // 检查是否在 Windows 上构建
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let out_dir = env::var("OUT_DIR").unwrap();
        let out_dir = Path::new(&out_dir);
        let sysroot_dir = find_sysroot();
        let from_dir = Path::new(&sysroot_dir).join("bin");

        let filelist = {
            let mut list = WINDOWS_DEPENDENTS.to_vec();
                if env::var("DEBUG").is_ok() {
                    list.append(&mut WINDOWS_DEPENDENTS_DEBUG.to_vec());
                }
            list
        };
        for file in filelist {
            let from = from_dir.join(file);
            let to: std::path::PathBuf = out_dir.join(file);
            if Path::exists(&to) {
                println!("Skiped: {}", to.display());
                continue;
            }
            println!("Copying {} from to {}", from.display(), to.display());
            fs::copy(&from, &to).unwrap();
            println!("Copied: {}", to.display());
        }
    
        // 打印一个消息，表明拷贝完成
        println!("Dlls copied to {}", out_dir.to_str().unwrap());
    } else {
        println!("Running on a non-Windows platform.");
    }
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