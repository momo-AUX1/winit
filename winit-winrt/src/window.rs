use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use dpi::{LogicalSize, PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event::WindowEvent;
use winit_core::monitor::MonitorHandle as CoreMonitorHandle;
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindowTrait, WindowAttributes, WindowButtons, WindowId,
    WindowLevel,
};

use windows::core::Interface;
use windows::Foundation::Size as WinRtSize;
use windows::UI::Core::{CoreCursor, CoreCursorType, CoreWindow as WinRtCoreWindow};
use windows::UI::ViewManagement::ApplicationView;

use crate::cursor::cursor_icon_to_core;
use crate::event_loop::Runner;

pub struct Window {
    runner: Arc<Runner>,
    id: WindowId,
    cursor_visible: AtomicBool,
    cursor_icon: Mutex<CoreCursorType>,
}

impl std::fmt::Debug for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Window").finish_non_exhaustive()
    }
}

impl Window {
    pub(crate) fn new(
        runner: Arc<Runner>,
        _attributes: WindowAttributes,
    ) -> Result<Self, RequestError> {
        if runner.window_created.swap(true, Ordering::SeqCst) {
            return Err(NotSupportedError::new("WinRT only supports a single window").into());
        }

        if runner.core_window().is_none() {
            return Err(NotSupportedError::new("CoreWindow is not available yet").into());
        }

        Ok(Self {
            runner,
            id: WindowId::from_raw(0),
            cursor_visible: AtomicBool::new(true),
            cursor_icon: Mutex::new(CoreCursorType::Arrow),
        })
    }

    pub(crate) fn core_window(&self) -> WinRtCoreWindow {
        self.runner
            .core_window()
            .expect("CoreWindow must be available on WinRT")
    }

    fn set_core_cursor(&self, cursor_type: CoreCursorType) {
        let Ok(cursor) = CoreCursor::CreateCursor(cursor_type, 0) else {
            return;
        };
        if let Some(window) = self.runner.core_window() {
            let _ = window.SetPointerCursor(&cursor);
        }
    }
}

impl rwh_06::HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::WindowsDisplayHandle::new();
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw.into()) })
    }
}

impl rwh_06::HasWindowHandle for Window {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let Some(window) = self.runner.core_window() else {
            return Err(rwh_06::HandleError::Unavailable);
        };
        let raw = window.as_raw();
        let Some(raw) = NonNull::new(raw as *mut std::ffi::c_void) else {
            return Err(rwh_06::HandleError::Unavailable);
        };
        let handle = rwh_06::WinRtWindowHandle::new(raw);
        Ok(unsafe { rwh_06::WindowHandle::borrow_raw(handle.into()) })
    }
}

impl CoreWindowTrait for Window {
    fn id(&self) -> WindowId {
        self.id
    }

    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        Some(self.runner.monitor_handle().to_core())
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(std::iter::once(self.runner.monitor_handle().to_core()))
    }

    fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        Some(self.runner.monitor_handle().to_core())
    }

    fn scale_factor(&self) -> f64 {
        self.runner.scale_factor()
    }

    fn request_redraw(&self) {
        self.runner.queue_window_event(WindowEvent::RedrawRequested);
        self.runner.wake_up();
    }

    fn pre_present_notify(&self) {}

    fn reset_dead_keys(&self) {}

    fn surface_position(&self) -> PhysicalPosition<i32> {
        (0, 0).into()
    }

    fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        Err(NotSupportedError::new("outer_position is not supported").into())
    }

    fn set_outer_position(&self, _position: Position) {
        // no-op on WinRT
    }

    fn surface_size(&self) -> PhysicalSize<u32> {
        self.runner.surface_size()
    }

    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let scale_factor = self.scale_factor();
        let logical = size.to_logical::<f64>(scale_factor);
        if let Ok(view) = ApplicationView::GetForCurrentView() {
            let winrt_size = WinRtSize {
                Width: logical.width as f32,
                Height: logical.height as f32,
            };
            if view.TryResizeView(winrt_size).ok().unwrap_or(false) {
                return None;
            }
        }
        Some(self.surface_size())
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        self.surface_size()
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        let Ok(view) = ApplicationView::GetForCurrentView() else {
            return PhysicalInsets::new(0, 0, 0, 0);
        };
        let Ok(visible) = view.VisibleBounds() else {
            return PhysicalInsets::new(0, 0, 0, 0);
        };
        let Ok(bounds) = self.core_window().Bounds() else {
            return PhysicalInsets::new(0, 0, 0, 0);
        };

        let left = (visible.X - bounds.X).max(0.0) as f64;
        let top = (visible.Y - bounds.Y).max(0.0) as f64;
        let right = ((bounds.X + bounds.Width) - (visible.X + visible.Width)).max(0.0) as f64;
        let bottom = ((bounds.Y + bounds.Height) - (visible.Y + visible.Height)).max(0.0) as f64;

        let scale_factor = self.scale_factor();
        PhysicalInsets::new(
            (left * scale_factor).round() as u32,
            (top * scale_factor).round() as u32,
            (right * scale_factor).round() as u32,
            (bottom * scale_factor).round() as u32,
        )
    }

    fn set_min_surface_size(&self, min_size: Option<Size>) {
        let Ok(view) = ApplicationView::GetForCurrentView() else {
            return;
        };
        let scale_factor = self.scale_factor();
        let logical = min_size
            .unwrap_or_else(|| Size::new(LogicalSize::new(0.0, 0.0)))
            .to_logical::<f64>(scale_factor);
        let winrt_size = WinRtSize {
            Width: logical.width as f32,
            Height: logical.height as f32,
        };
        let _ = view.SetPreferredMinSize(winrt_size);
    }

    fn set_max_surface_size(&self, _max_size: Option<Size>) {}

    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    fn set_surface_resize_increments(&self, _increments: Option<Size>) {}

    fn set_title(&self, _title: &str) {}

    fn set_transparent(&self, _transparent: bool) {}

    fn set_blur(&self, _blur: bool) {}

    fn set_visible(&self, _visible: bool) {}

    fn is_visible(&self) -> Option<bool> {
        self.runner.core_window().and_then(|window| window.Visible().ok())
    }

    fn set_resizable(&self, _resizable: bool) {}

    fn is_resizable(&self) -> bool {
        false
    }

    fn set_enabled_buttons(&self, _buttons: WindowButtons) {}

    fn enabled_buttons(&self) -> WindowButtons {
        WindowButtons::all()
    }

    fn set_minimized(&self, _minimized: bool) {}

    fn is_minimized(&self) -> Option<bool> {
        None
    }

    fn set_maximized(&self, _maximized: bool) {}

    fn is_maximized(&self) -> bool {
        false
    }

    fn set_fullscreen(&self, monitor: Option<winit_core::monitor::Fullscreen>) {
        let Ok(view) = ApplicationView::GetForCurrentView() else {
            return;
        };
        if monitor.is_some() {
            let _ = view.TryEnterFullScreenMode();
        } else {
            let _ = view.ExitFullScreenMode();
        }
    }

    fn fullscreen(&self) -> Option<winit_core::monitor::Fullscreen> {
        let Ok(view) = ApplicationView::GetForCurrentView() else {
            return None;
        };
        if view.IsFullScreenMode().ok().unwrap_or(false) {
            Some(winit_core::monitor::Fullscreen::Borderless(None))
        } else {
            None
        }
    }

    fn set_decorations(&self, _decorations: bool) {}

    fn is_decorated(&self) -> bool {
        true
    }

    fn set_window_level(&self, _level: WindowLevel) {}

    fn set_window_icon(&self, _window_icon: Option<winit_core::icon::Icon>) {}

    fn set_ime_cursor_area(&self, _position: Position, _size: Size) {}

    fn request_ime_update(&self, _request: ImeRequest) -> Result<(), ImeRequestError> {
        Err(ImeRequestError::NotSupported)
    }

    fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        None
    }

    fn set_ime_purpose(&self, _purpose: winit_core::window::ImePurpose) {}

    fn focus_window(&self) {
        if let Some(window) = self.runner.core_window() {
            let _ = window.Activate();
        }
    }

    fn has_focus(&self) -> bool {
        self.runner.has_focus()
    }

    fn request_user_attention(&self, _request_type: Option<UserAttentionType>) {}

    fn set_cursor(&self, cursor: Cursor) {
        if let Cursor::Icon(icon) = cursor {
            let core = cursor_icon_to_core(icon);
            *self.cursor_icon.lock().unwrap() = core;
            if self.cursor_visible.load(Ordering::SeqCst) {
                self.set_core_cursor(core);
            }
        }
    }

    fn set_cursor_position(&self, _position: Position) -> Result<(), RequestError> {
        Err(NotSupportedError::new("set_cursor_position is not supported").into())
    }

    fn set_cursor_grab(&self, _mode: CursorGrabMode) -> Result<(), RequestError> {
        Err(NotSupportedError::new("set_cursor_grab is not supported").into())
    }

    fn set_cursor_visible(&self, visible: bool) {
        self.cursor_visible.store(visible, Ordering::SeqCst);
        if visible {
            let icon = *self.cursor_icon.lock().unwrap();
            self.set_core_cursor(icon);
        } else {
            if let Some(window) = self.runner.core_window() {
                let _ = window.SetPointerCursor(None::<&CoreCursor>);
            }
        }
    }

    fn drag_window(&self) -> Result<(), RequestError> {
        Err(NotSupportedError::new("drag_window is not supported").into())
    }

    fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), RequestError> {
        Err(NotSupportedError::new("drag_resize_window is not supported").into())
    }

    fn show_window_menu(&self, _position: Position) {}

    fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), RequestError> {
        Err(NotSupportedError::new("set_cursor_hittest is not supported").into())
    }

    fn set_theme(&self, _theme: Option<Theme>) {}

    fn theme(&self) -> Option<Theme> {
        None
    }

    fn set_content_protected(&self, protected: bool) {
        let Ok(view) = ApplicationView::GetForCurrentView() else {
            return;
        };
        let _ = view.SetIsScreenCaptureEnabled(!protected);
    }

    fn title(&self) -> String {
        String::new()
    }

    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }

    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        self
    }
}
