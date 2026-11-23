//! Window-chrome manager shared by any Slint window that opts into the
//! Win32 custom client frame. UI components such as `UiTopBar` only emit
//! callbacks; the host routes those callbacks to these window operations.

use crate::ui_slint_win32_frame::{self, FrameConfig};

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub frame: FrameConfig,
    pub corner_radius: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self { frame: FrameConfig::default(), corner_radius: 0.0 }
    }
}

/// Owns the policy for one client-side Win32 frame. It deliberately does not
/// own application commands or UI data models.
#[derive(Clone, Copy, Debug)]
pub struct UiSlintWin32Control {
    options: Options,
}

impl UiSlintWin32Control {
    /// Must run once before any Slint Window is constructed.
    pub fn select_backend() -> Result<(), slint::PlatformError> {
        ui_slint_win32_frame::select_alpha_winit_backend()
    }

    pub fn new(options: Options) -> Self { Self { options } }

    /// Registers the deferred HWND hook. Call before `ComponentHandle::show`.
    pub fn install(&self, window: &slint::Window) {
        ui_slint_win32_frame::install(window, self.options.frame);
    }

    /// Finish native setup after the window exists. Safe to call on every
    /// show, which is important for detached popup windows.
    pub fn after_show(&self, window: &slint::Window) {
        ui_slint_win32_frame::install_now(window, self.options.frame);
        ui_slint_win32_frame::set_corner_radius(window, self.options.corner_radius);
    }

    pub fn set_corner_radius(&mut self, window: &slint::Window, radius: f32) {
        self.options.corner_radius = radius.max(0.0);
        ui_slint_win32_frame::set_corner_radius(window, self.options.corner_radius);
    }

    pub fn minimize(&self, window: &slint::Window) { window.set_minimized(true); }

    pub fn toggle_maximized(&self, window: &slint::Window) {
        window.set_maximized(!window.is_maximized());
    }
}
