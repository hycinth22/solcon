Currently supports rust nightly-2024-05-13, rustc 1.80.0-nightly (ef0027897 2024-05-12)

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Rust toolchain & required components
1. rustup toolchain install nightly-2024-05-13
2. rustup +nightly-2024-05-13 component add rust-src
3. rustup +nightly-2024-05-13 component add rustc-dev
4. rustup +nightly-2024-05-13 component add llvm-tools-preview

# Build & Install solcon_instrumenter using nightly
1. git clone https://github.com/hycinth22/solcon solcon_instrumenter
2. cd solcon_instrumenter/instrumenter
3. export RUST_SYSROOT=$(rustc +nightly-2024-05-13 --print sysroot)
4. cargo +nightly-2024-05-13 build
5. cargo install --path .

# Configure solcon_instrumenter
1. export SOLCON_MONITOR_LIB_PATH="$(pwd)/this_is_our_monitor_function/target/debug/libthis_is_our_monitor_function.rlib"
2. export SOLCON_LOG="info"
3. export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$RUST_SYSROOT/lib"

# Build monitor
1. cd this_is_our_monitor_function
2. ./build_monitor.sh

# Replace rustc with solcon_instrumenter & Build using our tool
1. export RUSTC_WRAPPER=~/.cargo/bin/solcon_instrumenter
Now, cargo will invoke our insturmenter instead of call rustc.
2. cd /path/to/your/project/you/want/instrument
3. cargo +nightly-2024-05-13 build

After you finished and dont need solcon_instrumenter, run `export RUSTC_WRAPPER=""` to resume original Rust compiler.
