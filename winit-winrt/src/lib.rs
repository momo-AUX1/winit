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
mod util;
mod window;

pub use event_loop::{ActiveEventLoop, EventLoop, PlatformSpecificEventLoopAttributes};
pub use monitor::MonitorHandle;
pub use window::Window;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use windows::UI::Core::{CoreDispatcher, CoreWindow as WinRtCoreWindow};
use winit_core::event_loop::ActiveEventLoop as CoreActiveEventLoop;
use winit_core::keyboard::{NativeKeyCode, PhysicalKey};
use winit_core::window::Window as CoreWindow;

/// Compatibility enum for Windows backdrop requests.
///
/// On WinRT/UWP these values are accepted but ignored.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub enum BackdropType {
    #[default]
    Auto = 0,
    None = 1,
    MainWindow = 2,
    TransientWindow = 3,
    TabbedWindow = 4,
}

/// Compatibility color type for Windows titlebar/border customization requests.
///
/// On WinRT/UWP this value is accepted but ignored.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Color(u32);

impl Color {
    pub const SYSTEM_DEFAULT: Color = Color(0xffff_ffff);

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::SYSTEM_DEFAULT
    }
}

/// Compatibility enum for Windows rounded-corner preferences.
///
/// On WinRT/UWP these values are accepted but ignored.
#[repr(i32)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub enum CornerPreference {
    #[default]
    Default = 0,
    DoNotRound = 1,
    Round = 2,
    RoundSmall = 3,
}

/// Additional methods on [`ActiveEventLoop`] that are specific to WinRT/UWP.
pub trait EventLoopExtWinRt {
    /// Returns the `CoreDispatcher` associated with the current view, if available.
    fn dispatcher(&self) -> Option<CoreDispatcher>;
}

/// Additional methods on [`Window`] that are specific to WinRT/UWP.
pub trait WindowExtWinRt {
    /// Returns the underlying `CoreWindow`.
    fn core_window(&self) -> WinRtCoreWindow;

    /// Compatibility shim for Win32 DWM API. No-op on WinRT/UWP.
    fn set_undecorated_shadow(&self, shadow: bool);

    /// Compatibility shim for Win32 DWM API. No-op on WinRT/UWP.
    fn set_system_backdrop(&self, backdrop_type: BackdropType);

    /// Compatibility shim for Win32 DWM API. No-op on WinRT/UWP.
    fn set_border_color(&self, color: Option<Color>);

    /// Compatibility shim for Win32 DWM API. No-op on WinRT/UWP.
    fn set_title_background_color(&self, color: Option<Color>);

    /// Compatibility shim for Win32 DWM API. No-op on WinRT/UWP.
    fn set_title_text_color(&self, color: Color);

    /// Compatibility shim for Win32 DWM API. No-op on WinRT/UWP.
    fn set_corner_preference(&self, preference: CornerPreference);
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

    fn set_undecorated_shadow(&self, shadow: bool) {
        let _ = shadow;
    }

    fn set_system_backdrop(&self, backdrop_type: BackdropType) {
        let _ = backdrop_type;
    }

    fn set_border_color(&self, color: Option<Color>) {
        let _ = color;
    }

    fn set_title_background_color(&self, color: Option<Color>) {
        let _ = color;
    }

    fn set_title_text_color(&self, color: Color) {
        let _ = color;
    }

    fn set_corner_preference(&self, preference: CornerPreference) {
        let _ = preference;
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
