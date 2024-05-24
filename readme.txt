Currently supports rust that latest update on 2024-05-22, rust version 1.80.0-nightly (791adf759 2024-05-21)

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Rust toolchain & needed components
1. rustup toolchain install nightly-2024-05-22
2. rustup +nightly-2024-05-22 component add rust-src
3. rustup +nightly-2024-05-22 component add rustc-dev
4. rustup +nightly-2024-05-22 component add llvm-tools-preview

# Install & Configure solcon_instrumenter
1. git clone https://github.com/hycinth22/solcon_instrumenter
2. cd solcon_instrumenter
3. cargo build
4. cargo install --path .
5. cd this_is_our_monitor_function
6. cargo build
7. export SOLCON_MONITOR_LIB_PATH="$(pwd)/this_is_our_monitor_function/target/debug/libthis_is_our_monitor_function.rlib"
8. export SOLCON_LOG="info"

# Attach solcon_instrumenter to rustc
1. export RUST_SYSROOT=$(rustc --print sysroot)
2. export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$RUST_SYSROOT/lib"
3. export RUSTC_WRAPPER=~/.cargo/bin/solcon_instrumenter

Now, you can switch to your program directory & simply use `cargo build` to build any Rust program, and solcon_instrumenter will automatically instrument it when compiling.

After you completed and dont need solcon_instrumenter, run `export RUSTC_WRAPPER=""` to resume original Rust compiler.
