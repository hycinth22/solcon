export RUSTC_LOG="solcon_instrumenter=info"
export RUSTC_WRAPPER="/home/hycinth/src/solcon_instrumenter/target/debug/solcon_instrumenter"
export RUSTFLAGS="--extern this_is_our_monitor_function=$SOLCON_MONITOR_LIB_PATH -L dependency=$(dirname $SOLCON_MONITOR_LIB_PATH)"
export RUSTDOCFLAGS="--extern this_is_our_monitor_function=$SOLCON_MONITOR_LIB_PATH -L dependency=$(dirname $SOLCON_MONITOR_LIB_PATH)"
export SOLCON_LOG="debug"
export SOLCON_LOG_COLOR="auto"
export SOLCON_MONITOR_LIB_PATH="/home/hycinth/src/solcon_instrumenter/this_is_our_monitor_function/target/debug/libthis_is_our_monitor_function.rlib"

#export TOOLCHAIN_NAME="nightly"
#export TOOLCHAIN_NAME="stage1"
#export TOOLCHAIN_NAME="stage2"
#export RUST_SYSROOT=$(rustc +$TOOLCHAIN_NAME --print=sysroot)
#export RUST_SYSROOT=$(rustc +$TOOLCHAIN_NAME --print=sysroot)
#export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$RUST_SYSROOT/lib/"