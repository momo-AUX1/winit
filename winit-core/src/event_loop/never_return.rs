use crate::application::ApplicationHandler;

/// Additional methods on `EventLoop` for platforms whose run method never return.
pub trait EventLoopExtNeverReturn {
    /// Run the event loop and call `process::exit` once finished.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS**: This registers the callbacks with the system and calls `UIApplicationMain`.
    /// - **WinRT/UWP**: This registers the callbacks with the system and calls `CoreApplication::Run`.
    /// - **macOS**: Unimplemented (TODO: Should call `NSApplicationMain`).
    /// - **Android/Orbital/Wayland/Windows (Win32)/X11**: Unsupported.
    /// - **Web**: Impossible to support properly.
    fn run_app_never_return<A: ApplicationHandler + 'static>(self, app: A) -> !;
}
