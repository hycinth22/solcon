Currently supports rustc 1.80.0-nightly (9cdfe285ca724c801dc9f78d22b24ea69b787f26 2024-05-22 LLVM version: 18.1.6)

1. git clone
2. cd solcon_instrumenter
3. rustup toolchain install nightly
4. rustup component add rust-src rustc-dev llvm-tools-preview
5. cargo install --path .
6. export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$(rustc --print sysroot)/lib"
6. export RUSTC_WRAPPER=
7. export RUSTC_LOG="solcon_instructmenter=info"