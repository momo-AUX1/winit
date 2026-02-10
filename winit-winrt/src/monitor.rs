use std::borrow::Cow;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;

use dpi::{PhysicalPosition, PhysicalSize};
use winit_core::monitor::{MonitorHandle as RootMonitorHandle, MonitorHandleProvider, VideoMode};

use windows::core::AgileReference;
use windows::Graphics::Display::DisplayInformation;
use windows::Graphics::Display::Core::HdmiDisplayInformation;

use crate::util::ensure_winrt_initialized;

#[derive(Debug, Clone)]
pub struct MonitorHandle {
    scale_factor: f64,
    display_info: Option<AgileReference<DisplayInformation>>,
}

impl MonitorHandle {
    pub(crate) fn new(
        scale_factor: f64,
        display_info: Option<AgileReference<DisplayInformation>>,
    ) -> Self {
        Self { scale_factor, display_info }
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
        ensure_winrt_initialized();
        let info = self.display_info.as_ref()?.resolve().ok()?;
        let width = info.ScreenWidthInRawPixels().ok()?;
        let height = info.ScreenHeightInRawPixels().ok()?;

        let (bit_depth, refresh_rate_millihertz) = HdmiDisplayInformation::GetForCurrentView()
            .ok()
            .and_then(|hdi| hdi.GetCurrentDisplayMode().ok())
            .map(|mode| {
                let bit_depth = mode
                    .BitsPerPixel()
                    .ok()
                    .and_then(|bpp| u16::try_from(bpp).ok())
                    .and_then(NonZeroU16::new);

                let refresh_rate_millihertz = mode.RefreshRate().ok().and_then(|hz| {
                    let hz = hz as f64;
                    let mhz = (hz * 1000.0).round();
                    if mhz.is_finite() && mhz > 0.0 && mhz <= u32::MAX as f64 {
                        NonZeroU32::new(mhz as u32)
                    } else {
                        None
                    }
                });

                (bit_depth, refresh_rate_millihertz)
            })
            .unwrap_or((None, None));

        Some(VideoMode::new(
            PhysicalSize::new(width, height),
            bit_depth,
            refresh_rate_millihertz,
        ))
    }

    fn video_modes(&self) -> Box<dyn Iterator<Item = VideoMode>> {
        Box::new(self.current_video_mode().into_iter())
    }
}
