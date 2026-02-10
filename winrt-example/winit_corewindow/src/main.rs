#![no_main]

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

struct App {
    window: Option<Box<dyn Window>>,
}

impl Default for App {
    fn default() -> Self {
        Self { window: None }
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.window = Some(event_loop.create_window(WindowAttributes::default()).unwrap());
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {}
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &dyn ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn entry() -> i32 {
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };

    let event_loop = EventLoop::new().unwrap();
    let _ = event_loop.run_app(App::default());
    0
}

#[no_mangle]
pub extern "system" fn wWinMain(
    _instance: isize,
    _prev_instance: isize,
    _cmd_line: *mut u16,
    _show_cmd: i32,
) -> i32 {
    entry()
}

// MinGW defaults to expecting `WinMain` unless linked with `-municode`. Provide both entry points
// so the example links in either configuration.
#[no_mangle]
pub extern "system" fn WinMain(
    _instance: isize,
    _prev_instance: isize,
    _cmd_line: *mut u8,
    _show_cmd: i32,
) -> i32 {
    entry()
}
