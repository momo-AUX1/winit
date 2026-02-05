use std::borrow::Cow;
use std::sync::Arc;

use dpi::PhysicalPosition;
use winit_core::monitor::{MonitorHandle as RootMonitorHandle, MonitorHandleProvider, VideoMode};

#[derive(Debug, Clone)]
pub struct MonitorHandle {
    scale_factor: f64,
}

impl MonitorHandle {
    pub(crate) fn new(scale_factor: f64) -> Self {
        Self { scale_factor }
    }

    pub(crate) fn to_core(self) -> RootMonitorHandle {
        RootMonitorHandle(Arc::new(self))
    }
}

impl MonitorHandleProvider for MonitorHandle {
    fn id(&self) -> u128 {
        0
    }

    fn native_id(&self) -> u64 {
        0
    }

    fn name(&self) -> Option<Cow<'_, str>> {
        None
    }

    fn position(&self) -> Option<PhysicalPosition<i32>> {
        None
    }

    fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    fn current_video_mode(&self) -> Option<VideoMode> {
        None
    }

    fn video_modes(&self) -> Box<dyn Iterator<Item = VideoMode>> {
        Box::new(std::iter::empty())
    }
}
