//! WinRT/UWP backend for winit.
#![cfg(all(target_os = "windows", __WINRT__))]
#![allow(non_snake_case)]

#[cfg(target_env = "msvc")]
compile_error!(
    "WinRT backend requires the GNU toolchain. Use target x86_64-pc-windows-gnu with \
     cfg(__WINRT__)."
);

mod cursor;
mod event_loop;
mod monitor;
mod window;

pub use event_loop::{ActiveEventLoop, EventLoop, PlatformSpecificEventLoopAttributes};
pub use monitor::MonitorHandle;
pub use window::Window;
use windows::UI::Core::{CoreDispatcher, CoreWindow as WinRtCoreWindow};
use winit_core::event_loop::ActiveEventLoop as CoreActiveEventLoop;
use winit_core::keyboard::{NativeKeyCode, PhysicalKey};
use winit_core::window::Window as CoreWindow;

/// Additional methods on [`ActiveEventLoop`] that are specific to WinRT/UWP.
pub trait EventLoopExtWinRt {
    /// Returns the `CoreDispatcher` associated with the current view, if available.
    fn dispatcher(&self) -> Option<CoreDispatcher>;
}

/// Additional methods on [`Window`] that are specific to WinRT/UWP.
pub trait WindowExtWinRt {
    /// Returns the underlying `CoreWindow`.
    fn core_window(&self) -> WinRtCoreWindow;
}

impl EventLoopExtWinRt for dyn CoreActiveEventLoop + '_ {
    fn dispatcher(&self) -> Option<CoreDispatcher> {
        let event_loop = self.cast_ref::<ActiveEventLoop>().unwrap();
        event_loop.dispatcher()
    }
}

impl WindowExtWinRt for dyn CoreWindow + '_ {
    fn core_window(&self) -> WinRtCoreWindow {
        let window = self.cast_ref::<Window>().unwrap();
        window.core_window()
    }
}

pub fn physicalkey_to_scancode(physical_key: PhysicalKey) -> Option<u32> {
    match physical_key {
        PhysicalKey::Unidentified(NativeKeyCode::Windows(scancode)) => Some(scancode as u32),
        _ => None,
    }
}

pub fn scancode_to_physicalkey(scancode: u32) -> PhysicalKey {
    PhysicalKey::Unidentified(NativeKeyCode::Windows(scancode as u16))
}
