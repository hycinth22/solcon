export SOLCON_LOG="debug"
export SOLCON_LOG_COLOR="auto"
export SOLCON_MONITOR_LIB_PATH="/home/hycinth/src/solcon_instrumenter/this_is_our_monitor_function/target/debug/libthis_is_our_monitor_function.rlib"
# export RUST_SYSROOT=$(rustc +nightly --print=sysroot)
# export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$RUST_SYSROOT/lib/"
export RUSTC_LOG="warn"
export RUSTC_WRAPPER="/home/hycinth/src/solcon_instrumenter/target/debug/solcon_instrumenter"
export RUSTFLAGS="--extern this_is_our_monitor_function=$SOLCON_MONITOR_LIB_PATH -L dependency=$(dirname $SOLCON_MONITOR_LIB_PATH)"
export RUSTDOCFLAGS="--extern this_is_our_monitor_function=$SOLCON_MONITOR_LIB_PATH -L dependency=$(dirname $SOLCON_MONITOR_LIB_PATH)"
