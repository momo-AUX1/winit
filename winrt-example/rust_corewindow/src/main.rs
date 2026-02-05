#![no_main]

use std::cell::RefCell;
use std::thread::sleep;
use std::time::Duration;

use windows::core::{implement, Result, HSTRING};
use windows::ApplicationModel::Core::{
    CoreApplication, CoreApplicationView, IFrameworkView, IFrameworkViewSource,
    IFrameworkViewSource_Impl, IFrameworkView_Impl,
};
use windows::UI::Core::{CoreProcessEventsOption, CoreWindow};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

#[implement(IFrameworkViewSource, IFrameworkView)]
#[derive(Clone)]
struct App {
    window: RefCell<Option<CoreWindow>>,
}

impl App {
    fn new() -> Self {
        Self {
            window: RefCell::new(None),
        }
    }
}

#[allow(non_snake_case)]
impl IFrameworkViewSource_Impl for App {
    fn CreateView(&self) -> Result<IFrameworkView> {
        Ok(self.clone().into())
    }
}

#[allow(non_snake_case)]
impl IFrameworkView_Impl for App {
    fn Initialize(&self, _application_view: Option<&CoreApplicationView>) -> Result<()> {
        Ok(())
    }

    fn SetWindow(&self, window: Option<&CoreWindow>) -> Result<()> {
        if let Some(window) = window {
            *self.window.borrow_mut() = Some(window.clone());
            window.Activate()?;
        }
        Ok(())
    }

    fn Load(&self, _entry_point: &HSTRING) -> Result<()> {
        Ok(())
    }

    fn Run(&self) -> Result<()> {
        let window = loop {
            if let Some(window) = self.window.borrow().clone() {
                break window;
            }
            sleep(Duration::from_millis(10));
        };
        let dispatcher = window.Dispatcher()?;
        loop {
            dispatcher.ProcessEvents(CoreProcessEventsOption::ProcessAllIfPresent)?;
            sleep(Duration::from_millis(10));
        }
    }

    fn Uninitialize(&self) -> Result<()> {
        Ok(())
    }
}

#[no_mangle]
pub extern "system" fn wWinMain(
    _instance: isize,
    _prev_instance: isize,
    _cmd_line: *mut u16,
    _show_cmd: i32,
) -> i32 {
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    let app: IFrameworkViewSource = App::new().into();
    let _ = CoreApplication::Run(&app);
    0
}
