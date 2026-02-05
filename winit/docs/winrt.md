# WinRT / UWP (CoreWindow) backend

This backend targets WinRT/UWP using `CoreApplication` + `CoreWindow` and is selected by
compiling with `cfg(__WINRT__)` (see build instructions below). It is intentionally minimal and
focused on booting apps and delivering basic input events.

## Build

### Toolchain

- Target: `x86_64-pc-windows-gnu`
- Compile flag: `RUSTFLAGS="--cfg __WINRT__"`

### Example

```
RUSTFLAGS="--cfg __WINRT__" \
  cargo build -p winit-winrt --target x86_64-pc-windows-gnu
```

See `winrt-example/` for a runnable example and UWP packaging notes.

## Supported features (initial scope)

- Event loop boot via `CoreApplication::Run`
- Single window creation via `CoreWindow`
- Basic window events: resize, focus, close requested
- Pointer input: mouse/touch/pen (minimal)
- Keyboard input: basic key presses and text (minimal)
- Cursor icon + visibility (CoreCursor)
- Fullscreen (borderless via `ApplicationView::TryEnterFullScreenMode`)
- Best-effort surface resize requests (`ApplicationView::TryResizeView`)
- Preferred minimum size (`ApplicationView::SetPreferredMinSize`)
- Safe area insets (`ApplicationView::VisibleBounds`)
- Content protection (`ApplicationView::SetIsScreenCaptureEnabled`)

## Unsupported or no-op APIs

The following are not supported on WinRT/UWP and are implemented as no-ops or return
`RequestError::NotSupported`:

- Window movement/positioning
- Window title, decorations, resizable flags
- Max sizing constraints
- Window level / attention
- Window icon
- Cursor grab / cursor position / drag window / hittest
- IME
- Raw device events (e.g. gamepad via `Windows.Gaming.Input`)

## Notes

- Only a single window is supported in the initial backend.
- Some APIs may evolve as the backend matures.
