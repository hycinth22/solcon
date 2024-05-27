
fn get_sysroot_from_rustc() -> Option<String> {
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

fn main() {
    // set environment variable RUST_SYSROOT from rustc at compile-time
    if let Some(sysroot) = get_sysroot_from_rustc() {
        println!("cargo::rustc-env=RUST_SYSROOT={}", sysroot);
    }
}
