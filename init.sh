# export TOOLCHAIN_NAME="stage2"
export TOOLCHAIN_NAME="nightly"
export RUSTC_LOG="rustc_metadata=debug,solcon_instrumenter=debug"
export RUSTC_WRAPPER="/home/hycinth/src/solcon_instrumenter/target/debug/solcon_instrumenter"
export RUST_SYSROOT=$(rustc +$TOOLCHAIN_NAME --print=sysroot)
export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$RUST_SYSROOT/lib/"