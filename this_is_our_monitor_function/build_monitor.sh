#!/bin/bash

set -e

source ../init.sh
export SOLCON_BE_RUSTC=1
cargo build
