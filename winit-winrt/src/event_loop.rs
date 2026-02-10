use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use smol_str::SmolStr;
use windows::core::{implement, AgileReference, IInspectable, Result as WinResult};
use windows::ApplicationModel::Core::{
    CoreApplication, CoreApplicationView, IFrameworkView, IFrameworkViewSource,
    IFrameworkViewSource_Impl, IFrameworkView_Impl,
};
use windows::Devices::Input::PointerDeviceType;
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Display::DisplayInformation;
use windows::System::VirtualKey;
use windows::UI::Core::{
    CharacterReceivedEventArgs, CoreDispatcher, CoreDispatcherPriority, CoreProcessEventsOption,
    CoreVirtualKeyStates, CoreWindow as WinRtCoreWindow, CoreWindowActivationState,
    CoreWindowEventArgs, KeyEventArgs, PointerEventArgs, WindowActivatedEventArgs,
    WindowSizeChangedEventArgs,
};
use windows::UI::Input::{PointerPointProperties, PointerUpdateKind};
use winit_core::application::ApplicationHandler;
use winit_core::cursor::{CustomCursor, CustomCursorSource};
use winit_core::error::{EventLoopError, NotSupportedError, RequestError};
use winit_core::event::{
    ElementState, Modifiers, MouseButton, MouseScrollDelta, StartCause, TouchPhase, WindowEvent,
};
use winit_core::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents, EventLoopProxy as CoreProxy,
    EventLoopProxyProvider, OwnedDisplayHandle as CoreOwnedDisplayHandle,
};
use winit_core::keyboard::{
    Key, KeyLocation, ModifiersKeys, ModifiersState, NativeKeyCode, PhysicalKey,
};
use winit_core::monitor::MonitorHandle as CoreMonitorHandle;
use winit_core::window::{Window as CoreWindowTrait, WindowAttributes, WindowId};

use crate::monitor::MonitorHandle;
use crate::util::ensure_winrt_initialized;
use crate::window::Window;

const GLOBAL_WINDOW_ID: WindowId = WindowId::from_raw(0);

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PlatformSpecificEventLoopAttributes {}

pub struct EventLoop {
    runner: Arc<Runner>,
    window_target: ActiveEventLoop,
}

impl std::fmt::Debug for EventLoop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventLoop").finish_non_exhaustive()
    }
}

impl EventLoop {
    pub fn new(
        _attributes: &mut PlatformSpecificEventLoopAttributes,
    ) -> Result<Self, EventLoopError> {
        static EVENT_LOOP_CREATED: AtomicBool = AtomicBool::new(false);
        if EVENT_LOOP_CREATED.swap(true, Ordering::Relaxed) {
            return Err(EventLoopError::RecreationAttempt);
        }

        ensure_winrt_initialized();

        let runner = Arc::new(Runner::new());
        let window_target = ActiveEventLoop { runner: Arc::clone(&runner) };

        Ok(Self { runner, window_target })
    }

    pub fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.window_target
    }

    pub fn run_app_never_return<A: ApplicationHandler + 'static>(self, app: A) -> ! {
        self.runner.set_app(app);

        let view: IFrameworkViewSource = FrameworkViewSource::new(self.runner.clone()).into();
        let _ = CoreApplication::Run(&view);

        std::process::exit(0)
    }
}

pub struct ActiveEventLoop {
    pub(crate) runner: Arc<Runner>,
}

impl std::fmt::Debug for ActiveEventLoop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveEventLoop").finish_non_exhaustive()
    }
}

impl ActiveEventLoop {
    pub(crate) fn dispatcher(&self) -> Option<CoreDispatcher> {
        self.runner.dispatcher()
    }
}

impl RootActiveEventLoop for ActiveEventLoop {
    fn create_proxy(&self) -> CoreProxy {
        CoreProxy::new(Arc::new(EventLoopProxy { runner: Arc::clone(&self.runner) }))
    }

    fn create_window(
        &self,
        window_attributes: WindowAttributes,
    ) -> Result<Box<dyn CoreWindowTrait>, RequestError> {
        let window = Window::new(self.runner.clone(), window_attributes)?;
        Ok(Box::new(window))
    }

    fn create_custom_cursor(
        &self,
        _custom_cursor: CustomCursorSource,
    ) -> Result<CustomCursor, RequestError> {
        Err(NotSupportedError::new("custom cursors are not supported on WinRT").into())
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(std::iter::once(self.runner.monitor_handle().to_core()))
    }

    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        Some(self.runner.monitor_handle().to_core())
    }

    fn listen_device_events(&self, _allowed: DeviceEvents) {}

    fn system_theme(&self) -> Option<winit_core::window::Theme> {
        None
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        *self.runner.control_flow.lock().unwrap() = control_flow;
    }

    fn control_flow(&self) -> ControlFlow {
        *self.runner.control_flow.lock().unwrap()
    }

    fn exit(&self) {
        self.runner.exit.store(true, Ordering::SeqCst);
        let _ = CoreApplication::Exit();
    }

    fn exiting(&self) -> bool {
        self.runner.exit.load(Ordering::SeqCst)
    }

    fn owned_display_handle(&self) -> CoreOwnedDisplayHandle {
        CoreOwnedDisplayHandle::new(self.runner.clone())
    }

    fn rwh_06_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        &*self.runner
    }
}

struct EventLoopProxy {
    runner: Arc<Runner>,
}

impl std::fmt::Debug for EventLoopProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventLoopProxy").finish_non_exhaustive()
    }
}

impl EventLoopProxyProvider for EventLoopProxy {
    fn wake_up(&self) {
        self.runner.queue_wakeup();
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Window { window_id: WindowId, event: WindowEvent },
    WakeUp,
}

struct PendingKeyDown {
    scancode: u16,
    event: winit_core::event::KeyEvent,
}

#[derive(Clone, Copy)]
struct AppPtr(*mut (dyn ApplicationHandler + 'static));

unsafe impl Send for AppPtr {}
unsafe impl Sync for AppPtr {}

pub(crate) struct Runner {
    app: Mutex<Option<AppPtr>>,
    pub(crate) control_flow: Mutex<ControlFlow>,
    pub(crate) exit: AtomicBool,
    events: Mutex<VecDeque<Event>>,
    window: Mutex<Option<AgileReference<WinRtCoreWindow>>>,
    dispatcher: Mutex<Option<AgileReference<CoreDispatcher>>>,
    display_info: Mutex<Option<AgileReference<DisplayInformation>>>,
    surface_size: Mutex<PhysicalSize<u32>>,
    scale_factor_bits: AtomicU64,
    has_focus: AtomicBool,
    pub(crate) window_created: AtomicBool,
    wakeup_pending: AtomicBool,
    pending_keydown: Mutex<Option<PendingKeyDown>>,
}

impl Runner {
    fn new() -> Self {
        Self {
            app: Mutex::new(None),
            control_flow: Mutex::new(ControlFlow::default()),
            exit: AtomicBool::new(false),
            events: Mutex::new(VecDeque::new()),
            window: Mutex::new(None),
            dispatcher: Mutex::new(None),
            display_info: Mutex::new(None),
            surface_size: Mutex::new(PhysicalSize::new(0, 0)),
            scale_factor_bits: AtomicU64::new(f64::to_bits(1.0)),
            has_focus: AtomicBool::new(false),
            window_created: AtomicBool::new(false),
            wakeup_pending: AtomicBool::new(false),
            pending_keydown: Mutex::new(None),
        }
    }

    pub(crate) fn core_window(&self) -> Option<WinRtCoreWindow> {
        ensure_winrt_initialized();
        self.window.lock().unwrap().as_ref().and_then(|agile| agile.resolve().ok())
    }

    pub(crate) fn dispatcher(&self) -> Option<CoreDispatcher> {
        ensure_winrt_initialized();
        self.dispatcher.lock().unwrap().as_ref().and_then(|agile| agile.resolve().ok())
    }

    pub(crate) fn surface_size(&self) -> PhysicalSize<u32> {
        *self.surface_size.lock().unwrap()
    }

    pub(crate) fn scale_factor(&self) -> f64 {
        f64::from_bits(self.scale_factor_bits.load(Ordering::Relaxed))
    }

    pub(crate) fn monitor_handle(&self) -> MonitorHandle {
        MonitorHandle::new(self.scale_factor(), self.display_info.lock().unwrap().clone())
    }

    pub(crate) fn has_focus(&self) -> bool {
        self.has_focus.load(Ordering::Relaxed)
    }

    pub(crate) fn queue_event(&self, event: Event) {
        self.events.lock().unwrap().push_back(event);
    }

    pub(crate) fn queue_window_event(&self, event: WindowEvent) {
        self.queue_event(Event::Window { window_id: GLOBAL_WINDOW_ID, event });
    }

    pub(crate) fn wake_up(&self) {
        if let Some(dispatcher) = self.dispatcher() {
            let _ = dispatcher.RunAsync(
                CoreDispatcherPriority::Normal,
                &windows::UI::Core::DispatchedHandler::new(|| Ok(())),
            );
        }
    }

    pub(crate) fn queue_wakeup(&self) {
        if self.wakeup_pending.swap(true, Ordering::SeqCst) {
            return;
        }
        self.queue_event(Event::WakeUp);
        self.wake_up();
    }

    fn set_app<A: ApplicationHandler + 'static>(&self, app: A) {
        let mut slot = self.app.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let boxed: Box<dyn ApplicationHandler> = Box::new(app);
        *slot = Some(AppPtr(Box::into_raw(boxed)));
    }

    fn take_app(&self) -> Option<*mut (dyn ApplicationHandler + 'static)> {
        self.app.lock().unwrap().take().map(|ptr| ptr.0)
    }

    fn app_ptr(&self) -> Option<*mut (dyn ApplicationHandler + 'static)> {
        self.app.lock().unwrap().map(|ptr| ptr.0)
    }

    fn set_window(self: &Arc<Self>, window: WinRtCoreWindow) {
        ensure_winrt_initialized();
        if let Ok(agile) = AgileReference::new(&window) {
            *self.window.lock().unwrap() = Some(agile);
        }
        if let Ok(dispatcher) = window.Dispatcher() {
            *self.dispatcher.lock().unwrap() = AgileReference::new(&dispatcher).ok();
        }

        if let Ok(info) = DisplayInformation::GetForCurrentView() {
            let dpi = info.LogicalDpi().unwrap_or(96.0);
            let scale = dpi_to_scale_factor(dpi as f64);
            self.scale_factor_bits.store(f64::to_bits(scale), Ordering::Relaxed);
            *self.display_info.lock().unwrap() = AgileReference::new(&info).ok();
        }

        let bounds = window.Bounds().unwrap_or_default();
        let size = LogicalSize::new(bounds.Width as f64, bounds.Height as f64)
            .to_physical::<u32>(self.scale_factor());
        *self.surface_size.lock().unwrap() = size;

        self.register_window_handlers(&window);
        self.register_display_handlers();
    }

    fn register_window_handlers(self: &Arc<Self>, window: &WinRtCoreWindow) {
        let _ = window.Activated(
            &TypedEventHandler::<WinRtCoreWindow, WindowActivatedEventArgs>::new({
                let runner = Arc::clone(self);
                move |_, args| {
                    if let Some(args) = args {
                        let active =
                            args.WindowActivationState()? != CoreWindowActivationState::Deactivated;
                        runner.has_focus.store(active, Ordering::Relaxed);
                        runner.queue_window_event(WindowEvent::Focused(active));
                    }
                    Ok(())
                }
            }),
        );

        let _ = window.SizeChanged(
            &TypedEventHandler::<WinRtCoreWindow, WindowSizeChangedEventArgs>::new({
                let runner = Arc::clone(self);
                move |_, args| {
                    if let Some(args) = args {
                        runner.handle_size_changed(args);
                    }
                    Ok(())
                }
            }),
        );

        let _ = window.Closed(&TypedEventHandler::<WinRtCoreWindow, CoreWindowEventArgs>::new({
            let runner = Arc::clone(self);
            move |_, _| {
                runner.queue_window_event(WindowEvent::CloseRequested);
                Ok(())
            }
        }));

        let _ =
            window.PointerMoved(&TypedEventHandler::<WinRtCoreWindow, PointerEventArgs>::new({
                let runner = Arc::clone(self);
                move |_, args| {
                    if let Some(args) = args {
                        runner.handle_pointer_moved(args);
                    }
                    Ok(())
                }
            }));

        let _ =
            window.PointerPressed(&TypedEventHandler::<WinRtCoreWindow, PointerEventArgs>::new({
                let runner = Arc::clone(self);
                move |_, args| {
                    if let Some(args) = args {
                        runner.handle_pointer_button(args, ElementState::Pressed);
                    }
                    Ok(())
                }
            }));

        let _ =
            window.PointerReleased(&TypedEventHandler::<WinRtCoreWindow, PointerEventArgs>::new({
                let runner = Arc::clone(self);
                move |_, args| {
                    if let Some(args) = args {
                        runner.handle_pointer_button(args, ElementState::Released);
                    }
                    Ok(())
                }
            }));

        let _ =
            window.PointerEntered(&TypedEventHandler::<WinRtCoreWindow, PointerEventArgs>::new({
                let runner = Arc::clone(self);
                move |_, args| {
                    if let Some(args) = args {
                        runner.handle_pointer_entered(args);
                    }
                    Ok(())
                }
            }));

        let _ =
            window.PointerExited(&TypedEventHandler::<WinRtCoreWindow, PointerEventArgs>::new({
                let runner = Arc::clone(self);
                move |_, args| {
                    if let Some(args) = args {
                        runner.handle_pointer_exited(args);
                    }
                    Ok(())
                }
            }));

        let _ = window.PointerWheelChanged(
            &TypedEventHandler::<WinRtCoreWindow, PointerEventArgs>::new({
                let runner = Arc::clone(self);
                move |_, args| {
                    if let Some(args) = args {
                        runner.handle_pointer_wheel(args);
                    }
                    Ok(())
                }
            }),
        );

        let _ = window.KeyDown(&TypedEventHandler::<WinRtCoreWindow, KeyEventArgs>::new({
            let runner = Arc::clone(self);
            move |_, args| {
                if let Some(args) = args {
                    runner.handle_key(args, ElementState::Pressed);
                }
                Ok(())
            }
        }));

        let _ = window.KeyUp(&TypedEventHandler::<WinRtCoreWindow, KeyEventArgs>::new({
            let runner = Arc::clone(self);
            move |_, args| {
                if let Some(args) = args {
                    runner.handle_key(args, ElementState::Released);
                }
                Ok(())
            }
        }));

        let _ = window.CharacterReceived(&TypedEventHandler::<
            WinRtCoreWindow,
            CharacterReceivedEventArgs,
        >::new({
            let runner = Arc::clone(self);
            move |_, args| {
                if let Some(args) = args {
                    runner.handle_character_received(args);
                }
                Ok(())
            }
        }));
    }

    fn register_display_handlers(self: &Arc<Self>) {
        ensure_winrt_initialized();
        let Some(info) =
            self.display_info.lock().unwrap().as_ref().and_then(|agile| agile.resolve().ok())
        else {
            return;
        };
        let runner = Arc::clone(self);
        let _ = info.DpiChanged(&TypedEventHandler::<DisplayInformation, IInspectable>::new(
            move |_, _| {
                runner.handle_dpi_changed();
                Ok(())
            },
        ));
    }

    fn handle_size_changed(&self, args: &WindowSizeChangedEventArgs) {
        let size = args.Size().unwrap_or_default();
        let physical = LogicalSize::new(size.Width as f64, size.Height as f64)
            .to_physical::<u32>(self.scale_factor());
        *self.surface_size.lock().unwrap() = physical;
        self.queue_window_event(WindowEvent::SurfaceResized(physical));
    }

    fn handle_dpi_changed(&self) {
        ensure_winrt_initialized();
        let Some(info) =
            self.display_info.lock().unwrap().as_ref().and_then(|agile| agile.resolve().ok())
        else {
            return;
        };
        let new_dpi = info.LogicalDpi().unwrap_or(96.0);
        let new_scale = dpi_to_scale_factor(new_dpi as f64);
        let old_scale = self.scale_factor();
        if (new_scale - old_scale).abs() < f64::EPSILON {
            return;
        }
        self.scale_factor_bits.store(f64::to_bits(new_scale), Ordering::Relaxed);

        let old_size = *self.surface_size.lock().unwrap();
        let new_size = old_size.to_logical::<f64>(old_scale).to_physical::<u32>(new_scale);
        let new_size_arc = Arc::new(Mutex::new(new_size));
        self.queue_window_event(WindowEvent::ScaleFactorChanged {
            scale_factor: new_scale,
            surface_size_writer: winit_core::event::SurfaceSizeWriter::new(Arc::downgrade(
                &new_size_arc,
            )),
        });
        let updated = *new_size_arc.lock().unwrap();
        *self.surface_size.lock().unwrap() = updated;
    }

    fn handle_pointer_entered(&self, args: &PointerEventArgs) {
        let point = match args.CurrentPoint() {
            Ok(point) => point,
            Err(_) => return,
        };
        let (position, primary, _source, kind) = self.pointer_details(&point);
        self.queue_window_event(WindowEvent::PointerEntered {
            device_id: None,
            position,
            primary,
            kind,
        });
    }

    fn handle_pointer_exited(&self, args: &PointerEventArgs) {
        let point = match args.CurrentPoint() {
            Ok(point) => point,
            Err(_) => return,
        };
        let (_, primary, _, kind) = self.pointer_details(&point);
        self.queue_window_event(WindowEvent::PointerLeft {
            device_id: None,
            position: None,
            primary,
            kind,
        });
    }

    fn handle_pointer_moved(&self, args: &PointerEventArgs) {
        let point = match args.CurrentPoint() {
            Ok(point) => point,
            Err(_) => return,
        };
        let (position, primary, source, _) = self.pointer_details(&point);
        self.queue_window_event(WindowEvent::PointerMoved {
            device_id: None,
            position,
            primary,
            source,
        });
    }

    fn handle_pointer_button(&self, args: &PointerEventArgs, state: ElementState) {
        let point = match args.CurrentPoint() {
            Ok(point) => point,
            Err(_) => return,
        };
        let (position, primary, source, _) = self.pointer_details(&point);
        let props = point.Properties().ok();
        let button = button_source_from_point(props.as_ref(), &source);
        self.queue_window_event(WindowEvent::PointerButton {
            device_id: None,
            state,
            position,
            primary,
            button,
        });
    }

    fn handle_pointer_wheel(&self, args: &PointerEventArgs) {
        let point = match args.CurrentPoint() {
            Ok(point) => point,
            Err(_) => return,
        };
        let props = match point.Properties() {
            Ok(props) => props,
            Err(_) => return,
        };
        let delta = props.MouseWheelDelta().unwrap_or(0);
        let is_horizontal = props.IsHorizontalMouseWheel().unwrap_or(false);
        let line = delta as f32 / 120.0;
        let (x, y) = if is_horizontal { (line, 0.0) } else { (0.0, line) };
        self.queue_window_event(WindowEvent::MouseWheel {
            device_id: None,
            delta: MouseScrollDelta::LineDelta(x, y),
            phase: TouchPhase::Moved,
        });
    }

    fn handle_key(&self, args: &KeyEventArgs, state: ElementState) {
        let virtual_key = args.VirtualKey().unwrap_or(VirtualKey::None);
        let status = args.KeyStatus().unwrap_or_default();
        let scancode = status.ScanCode as u16;
        let repeat = status.RepeatCount > 1;

        let modifiers = self.current_modifiers();
        self.queue_window_event(WindowEvent::ModifiersChanged(modifiers));

        let (logical_key, text) = map_key(virtual_key, modifiers.state());
        let (key_without_modifiers, _) = map_key(virtual_key, ModifiersState::empty());
        let text_with_all_modifiers = text.clone();

        let mut event = winit_core::event::KeyEvent {
            physical_key: PhysicalKey::Unidentified(NativeKeyCode::Windows(scancode)),
            logical_key,
            text,
            location: KeyLocation::Standard,
            state,
            repeat,
            text_with_all_modifiers,
            key_without_modifiers,
        };

        if state == ElementState::Released {
            event.repeat = false;
            event.text = None;
            event.text_with_all_modifiers = None;
        }

        if state == ElementState::Pressed {
            let mods = modifiers.state();
            let expect_text = matches!(event.logical_key, Key::Character(_))
                && !mods.control_key()
                && !mods.alt_key()
                && !mods.meta_key();

            let pending_to_flush = self.pending_keydown.lock().unwrap().take();
            if let Some(pending) = pending_to_flush {
                self.queue_window_event(WindowEvent::KeyboardInput {
                    device_id: None,
                    event: pending.event,
                    is_synthetic: false,
                });
            }

            if expect_text {
                *self.pending_keydown.lock().unwrap() = Some(PendingKeyDown { scancode, event });
                return;
            }
        } else {
            let mut pending_lock = self.pending_keydown.lock().unwrap();
            if let Some(pending) = pending_lock.take() {
                if pending.scancode == scancode {
                    drop(pending_lock);
                    self.queue_window_event(WindowEvent::KeyboardInput {
                        device_id: None,
                        event: pending.event,
                        is_synthetic: false,
                    });
                } else {
                    *pending_lock = Some(pending);
                }
            }
        }

        self.queue_window_event(WindowEvent::KeyboardInput {
            device_id: None,
            event,
            is_synthetic: false,
        });
    }

    fn handle_character_received(&self, args: &CharacterReceivedEventArgs) {
        if let Ok(code) = args.KeyCode() {
            if let Some(ch) = std::char::from_u32(code) {
                let pending = self.pending_keydown.lock().unwrap().take();
                if let Some(mut pending) = pending {
                    let text = SmolStr::new(ch.to_string());
                    pending.event.logical_key = Key::Character(text.clone());
                    pending.event.text = Some(text.clone());
                    pending.event.text_with_all_modifiers = Some(text);
                    // pending.event.key_without_modifiers is kept from the original mapping.
                    // It represents the key without modifiers.
                    self.queue_window_event(WindowEvent::KeyboardInput {
                        device_id: None,
                        event: pending.event,
                        is_synthetic: false,
                    });
                }
            }
        }
    }

    fn current_modifiers(&self) -> Modifiers {
        let Some(window) = self.core_window() else {
            return Modifiers::new(ModifiersState::empty(), ModifiersKeys::empty());
        };
        let shift = key_down(&window, VirtualKey::Shift);
        let ctrl = key_down(&window, VirtualKey::Control);
        let alt = key_down(&window, VirtualKey::Menu);
        let meta = key_down(&window, VirtualKey::LeftWindows)
            || key_down(&window, VirtualKey::RightWindows);
        let mut state = ModifiersState::empty();
        if shift {
            state.insert(ModifiersState::SHIFT);
        }
        if ctrl {
            state.insert(ModifiersState::CONTROL);
        }
        if alt {
            state.insert(ModifiersState::ALT);
        }
        if meta {
            state.insert(ModifiersState::META);
        }
        Modifiers::new(state, ModifiersKeys::empty())
    }

    fn pointer_details(
        &self,
        point: &windows::UI::Input::PointerPoint,
    ) -> (
        PhysicalPosition<f64>,
        bool,
        winit_core::event::PointerSource,
        winit_core::event::PointerKind,
    ) {
        let position = point.Position().unwrap_or_default();
        let logical = LogicalPosition::new(position.X as f64, position.Y as f64);
        let physical = logical.to_physical::<f64>(self.scale_factor());
        let primary = point.Properties().ok().and_then(|p| p.IsPrimary().ok()).unwrap_or(true);

        let source = match point.PointerDevice().ok().and_then(|d| d.PointerDeviceType().ok()) {
            Some(PointerDeviceType::Mouse) => winit_core::event::PointerSource::Mouse,
            Some(PointerDeviceType::Touch) => winit_core::event::PointerSource::Touch {
                finger_id: winit_core::event::FingerId::from_raw(
                    point.PointerId().unwrap_or(0) as usize
                ),
                force: None,
            },
            Some(PointerDeviceType::Pen) => winit_core::event::PointerSource::TabletTool {
                kind: winit_core::event::TabletToolKind::Pen,
                data: winit_core::event::TabletToolData::default(),
            },
            _ => winit_core::event::PointerSource::Unknown,
        };
        let kind = winit_core::event::PointerKind::from(source.clone());
        (physical, primary, source, kind)
    }

    fn run_loop(self: &Arc<Self>) {
        let active = ActiveEventLoop { runner: Arc::clone(self) };
        let mut start_cause = StartCause::Init;

        loop {
            let app_ptr = self.app_ptr();
            if let Some(app_ptr) = app_ptr {
                unsafe { (&mut *app_ptr).new_events(&active, start_cause) };
                if start_cause == StartCause::Init {
                    unsafe { (&mut *app_ptr).can_create_surfaces(&active) };
                }
            }

            self.process_os_events();
            self.dispatch_events(&active);

            if let Some(app_ptr) = app_ptr {
                unsafe { (&mut *app_ptr).about_to_wait(&active) };
            }

            if self.exit.load(Ordering::SeqCst) {
                break;
            }

            start_cause = next_start_cause(*self.control_flow.lock().unwrap());
        }

        if let Some(app_ptr) = self.take_app() {
            unsafe {
                drop(Box::from_raw(app_ptr));
            }
        }
    }

    fn process_os_events(&self) {
        let Some(dispatcher) = self.dispatcher() else {
            return;
        };
        let control_flow = *self.control_flow.lock().unwrap();
        match control_flow {
            ControlFlow::Poll => {
                let _ = dispatcher.ProcessEvents(CoreProcessEventsOption::ProcessAllIfPresent);
            },
            ControlFlow::Wait => {
                let _ = dispatcher.ProcessEvents(CoreProcessEventsOption::ProcessOneAndAllPending);
            },
            ControlFlow::WaitUntil(instant) => {
                let now = std::time::Instant::now();
                if now < instant {
                    let duration = instant - now;
                    std::thread::sleep(duration);
                }
                let _ = dispatcher.ProcessEvents(CoreProcessEventsOption::ProcessAllIfPresent);
            },
        }
    }

    fn dispatch_events(&self, active: &ActiveEventLoop) {
        let mut queue = VecDeque::new();
        {
            let mut lock = self.events.lock().unwrap();
            std::mem::swap(&mut *lock, &mut queue);
        }
        let app_ptr = self.app_ptr();
        let Some(app_ptr) = app_ptr else {
            return;
        };

        for event in queue {
            unsafe {
                match event {
                    Event::Window { window_id, event } => {
                        (&mut *app_ptr).window_event(active, window_id, event)
                    },
                    Event::WakeUp => {
                        self.wakeup_pending.store(false, Ordering::SeqCst);
                        (&mut *app_ptr).proxy_wake_up(active)
                    },
                }
            }
        }
    }
}

impl rwh_06::HasDisplayHandle for Runner {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::WindowsDisplayHandle::new();
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw.into()) })
    }
}

#[implement(IFrameworkViewSource, IFrameworkView)]
#[derive(Clone)]
struct FrameworkViewSource {
    runner: Arc<Runner>,
}

impl FrameworkViewSource {
    fn new(runner: Arc<Runner>) -> Self {
        Self { runner }
    }
}

#[allow(non_snake_case)]
impl IFrameworkViewSource_Impl for FrameworkViewSource {
    fn CreateView(&self) -> WinResult<IFrameworkView> {
        Ok(self.clone().into())
    }
}

#[allow(non_snake_case)]
impl IFrameworkView_Impl for FrameworkViewSource {
    fn Initialize(&self, _application_view: Option<&CoreApplicationView>) -> WinResult<()> {
        Ok(())
    }

    fn SetWindow(&self, window: Option<&WinRtCoreWindow>) -> WinResult<()> {
        if let Some(window) = window {
            self.runner.set_window(window.clone());
            let _ = window.Activate();
        }
        Ok(())
    }

    fn Load(&self, _entry_point: &windows::core::HSTRING) -> WinResult<()> {
        Ok(())
    }

    fn Run(&self) -> WinResult<()> {
        self.runner.run_loop();
        Ok(())
    }

    fn Uninitialize(&self) -> WinResult<()> {
        Ok(())
    }
}

fn dpi_to_scale_factor(dpi: f64) -> f64 {
    dpi / 96.0
}

fn next_start_cause(control_flow: ControlFlow) -> StartCause {
    let now = std::time::Instant::now();
    match control_flow {
        ControlFlow::Poll => StartCause::Poll,
        ControlFlow::Wait => StartCause::WaitCancelled { start: now, requested_resume: None },
        ControlFlow::WaitUntil(instant) => {
            if now >= instant {
                StartCause::ResumeTimeReached { start: now, requested_resume: instant }
            } else {
                StartCause::WaitCancelled { start: now, requested_resume: Some(instant) }
            }
        },
    }
}

fn key_down(window: &WinRtCoreWindow, key: VirtualKey) -> bool {
    window.GetKeyState(key).map(|state| state.contains(CoreVirtualKeyStates::Down)).unwrap_or(false)
}

fn map_key(virtual_key: VirtualKey, modifiers: ModifiersState) -> (Key, Option<SmolStr>) {
    if let Some(named) = map_virtual_key_named(virtual_key) {
        let key = Key::Named(named);
        let text = key.to_text().map(SmolStr::new);
        return (key, text);
    }

    if let Some(ch) = map_virtual_key_char(virtual_key, modifiers.shift_key()) {
        let s = ch.to_string();
        return (Key::Character(SmolStr::new(s.clone())), Some(SmolStr::new(s)));
    }

    (
        Key::Unidentified(winit_core::keyboard::NativeKey::Windows(
            u16::try_from(virtual_key.0).unwrap_or_default(),
        )),
        None,
    )
}

fn map_virtual_key_named(virtual_key: VirtualKey) -> Option<winit_core::keyboard::NamedKey> {
    use winit_core::keyboard::NamedKey;
    match virtual_key {
        VirtualKey::Enter => Some(NamedKey::Enter),
        VirtualKey::Escape => Some(NamedKey::Escape),
        VirtualKey::Back => Some(NamedKey::Backspace),
        VirtualKey::Tab => Some(NamedKey::Tab),
        VirtualKey::Delete => Some(NamedKey::Delete),
        VirtualKey::Insert => Some(NamedKey::Insert),
        VirtualKey::Home => Some(NamedKey::Home),
        VirtualKey::End => Some(NamedKey::End),
        VirtualKey::PageUp => Some(NamedKey::PageUp),
        VirtualKey::PageDown => Some(NamedKey::PageDown),
        VirtualKey::Left => Some(NamedKey::ArrowLeft),
        VirtualKey::Right => Some(NamedKey::ArrowRight),
        VirtualKey::Up => Some(NamedKey::ArrowUp),
        VirtualKey::Down => Some(NamedKey::ArrowDown),
        VirtualKey::F1 => Some(NamedKey::F1),
        VirtualKey::F2 => Some(NamedKey::F2),
        VirtualKey::F3 => Some(NamedKey::F3),
        VirtualKey::F4 => Some(NamedKey::F4),
        VirtualKey::F5 => Some(NamedKey::F5),
        VirtualKey::F6 => Some(NamedKey::F6),
        VirtualKey::F7 => Some(NamedKey::F7),
        VirtualKey::F8 => Some(NamedKey::F8),
        VirtualKey::F9 => Some(NamedKey::F9),
        VirtualKey::F10 => Some(NamedKey::F10),
        VirtualKey::F11 => Some(NamedKey::F11),
        VirtualKey::F12 => Some(NamedKey::F12),
        _ => None,
    }
}

fn map_virtual_key_char(virtual_key: VirtualKey, shift: bool) -> Option<char> {
    match virtual_key {
        VirtualKey::A => Some(if shift { 'A' } else { 'a' }),
        VirtualKey::B => Some(if shift { 'B' } else { 'b' }),
        VirtualKey::C => Some(if shift { 'C' } else { 'c' }),
        VirtualKey::D => Some(if shift { 'D' } else { 'd' }),
        VirtualKey::E => Some(if shift { 'E' } else { 'e' }),
        VirtualKey::F => Some(if shift { 'F' } else { 'f' }),
        VirtualKey::G => Some(if shift { 'G' } else { 'g' }),
        VirtualKey::H => Some(if shift { 'H' } else { 'h' }),
        VirtualKey::I => Some(if shift { 'I' } else { 'i' }),
        VirtualKey::J => Some(if shift { 'J' } else { 'j' }),
        VirtualKey::K => Some(if shift { 'K' } else { 'k' }),
        VirtualKey::L => Some(if shift { 'L' } else { 'l' }),
        VirtualKey::M => Some(if shift { 'M' } else { 'm' }),
        VirtualKey::N => Some(if shift { 'N' } else { 'n' }),
        VirtualKey::O => Some(if shift { 'O' } else { 'o' }),
        VirtualKey::P => Some(if shift { 'P' } else { 'p' }),
        VirtualKey::Q => Some(if shift { 'Q' } else { 'q' }),
        VirtualKey::R => Some(if shift { 'R' } else { 'r' }),
        VirtualKey::S => Some(if shift { 'S' } else { 's' }),
        VirtualKey::T => Some(if shift { 'T' } else { 't' }),
        VirtualKey::U => Some(if shift { 'U' } else { 'u' }),
        VirtualKey::V => Some(if shift { 'V' } else { 'v' }),
        VirtualKey::W => Some(if shift { 'W' } else { 'w' }),
        VirtualKey::X => Some(if shift { 'X' } else { 'x' }),
        VirtualKey::Y => Some(if shift { 'Y' } else { 'y' }),
        VirtualKey::Z => Some(if shift { 'Z' } else { 'z' }),
        VirtualKey::Number0 => Some('0'),
        VirtualKey::Number1 => Some('1'),
        VirtualKey::Number2 => Some('2'),
        VirtualKey::Number3 => Some('3'),
        VirtualKey::Number4 => Some('4'),
        VirtualKey::Number5 => Some('5'),
        VirtualKey::Number6 => Some('6'),
        VirtualKey::Number7 => Some('7'),
        VirtualKey::Number8 => Some('8'),
        VirtualKey::Number9 => Some('9'),
        VirtualKey::Space => Some(' '),
        _ => None,
    }
}

fn button_source_from_point(
    props: Option<&PointerPointProperties>,
    source: &winit_core::event::PointerSource,
) -> winit_core::event::ButtonSource {
    use winit_core::event::{ButtonSource, TabletToolButton, TabletToolData, TabletToolKind};
    match source {
        winit_core::event::PointerSource::Mouse => {
            let update = props.and_then(|p| p.PointerUpdateKind().ok());
            let mouse = match update {
                Some(PointerUpdateKind::LeftButtonPressed)
                | Some(PointerUpdateKind::LeftButtonReleased) => MouseButton::Left,
                Some(PointerUpdateKind::RightButtonPressed)
                | Some(PointerUpdateKind::RightButtonReleased) => MouseButton::Right,
                Some(PointerUpdateKind::MiddleButtonPressed)
                | Some(PointerUpdateKind::MiddleButtonReleased) => MouseButton::Middle,
                Some(PointerUpdateKind::XButton1Pressed)
                | Some(PointerUpdateKind::XButton1Released) => MouseButton::Back,
                Some(PointerUpdateKind::XButton2Pressed)
                | Some(PointerUpdateKind::XButton2Released) => MouseButton::Forward,
                _ => MouseButton::Left,
            };
            ButtonSource::Mouse(mouse)
        },
        winit_core::event::PointerSource::Touch { finger_id, .. } => {
            ButtonSource::Touch { finger_id: *finger_id, force: None }
        },
        winit_core::event::PointerSource::TabletTool { .. } => ButtonSource::TabletTool {
            kind: TabletToolKind::Pen,
            button: TabletToolButton::Contact,
            data: TabletToolData::default(),
        },
        winit_core::event::PointerSource::Unknown => ButtonSource::Unknown(0),
    }
}
