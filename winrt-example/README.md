# WinRT examples

This folder contains WinRT/UWP samples for testing Windows CoreWindow support.

## winit_corewindow

A minimal Winit-based CoreWindow app.

Build (GNU toolchain, WinRT cfg):

```
RUSTFLAGS="--cfg __WINRT__" \
  cargo build --manifest-path winrt-example/winit_corewindow/Cargo.toml \
  --target x86_64-pc-windows-gnu
```

Packaging and deployment (makeappx/makepri or your existing pipeline) are required to run the
binary as a UWP app.

## rust_corewindow

A minimal "hello window" sample using the `windows` crate directly (no Winit).
