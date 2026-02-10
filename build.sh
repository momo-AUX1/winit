#!/usr/bin/env bash
set -euo pipefail

export RUSTFLAGS="--cfg __WINRT__"

cargo check -p winit --target x86_64-pc-windows-gnu
cargo build -p winit-winrt --target x86_64-pc-windows-gnu

# Example crates live under the workspace root but are excluded from the workspace.
cargo build --manifest-path winrt-example/winit_corewindow/Cargo.toml --target x86_64-pc-windows-gnu
cargo build --manifest-path winrt-example/rust_corewindow/Cargo.toml --target x86_64-pc-windows-gnu
