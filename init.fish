#!/usr/bin/env fish

set project_root (dirname (status -f))
set instrumenter_root $project_root/instrumenter
set -x SOLCON_LOG "debug"
set -x SOLCON_LOG_COLOR "auto"
set -x SOLCON_MONITOR_LIB_PATH "$project_root/runtime_library/target/debug/libthis_is_our_monitor_function.rlib"
# set RUST_SYSROOT (rustc +nightly --print=sysroot)
# set -x LD_LIBRARY_PATH "$LD_LIBRARY_PATH:$RUST_SYSROOT/lib/"
set -x RUSTC_LOG "warn"
set -x RUSTC_WRAPPER "$instrumenter_root/target/debug/solcon_instrumenter"
set -x RUSTFLAGS "--extern this_is_our_monitor_function=$SOLCON_MONITOR_LIB_PATH -L dependency=(dirname $SOLCON_MONITOR_LIB_PATH)"
set -x RUSTDOCFLAGS "--extern this_is_our_monitor_function=$SOLCON_MONITOR_LIB_PATH -L dependency=(dirname $SOLCON_MONITOR_LIB_PATH)"
