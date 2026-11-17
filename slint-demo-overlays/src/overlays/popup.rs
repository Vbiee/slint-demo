//! Owns a standalone Slint `Window`, so callers never embed popup UI in their layout.

use crate::DetachedPopup;
use slint::{
    winit_030::{
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
        WinitWindowAccessor,
    },
    ComponentHandle,
};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{FlashWindowEx, SetForegroundWindow, FLASHW_CAPTION, FLASHWINFO},
};

#[derive(Clone, Copy, Debug)]
pub enum FrameStyle {
    Native,
    CustomWin32(crate::ui_slint_win32_frame::FrameConfig),
}

#[derive(Clone, Copy, Debug)]
pub struct PopupOptions {
    pub frame: FrameStyle,
    /// Used only by `CustomWin32`; zero preserves the DWM shadow seam.
    pub corner_radius: f32,
}

impl Default for PopupOptions {
    fn default() -> Self {
        Self { frame: FrameStyle::Native, corner_radius: 0.0 }
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    Accepted,
    Dismissed,
}

pub struct PopupWindowManager {
    popup: DetachedPopup,
    custom_frame: Option<crate::ui_slint_win32_frame::FrameConfig>,
    custom_corner_radius: Option<f32>,
}

pub struct TopBarConfig { pub show_minimize: bool, pub show_maximize: bool, pub show_close: bool, pub height: f32, pub title_size: f32 }

impl PopupWindowManager {
    pub fn new(
        options: PopupOptions,
        on_event: impl Fn(Event) + 'static,
    ) -> Result<Self, slint::PlatformError> {
        let popup = DetachedPopup::new()?;
        let custom_frame = if let FrameStyle::CustomWin32(config) = options.frame {
            crate::ui_slint_win32_frame::install(&popup.window(), config);
            Some(config)
        } else {
            None
        };
        let custom_corner_radius = custom_frame.map(|_| options.corner_radius);
        let popup_weak = popup.as_weak();
        let on_event = std::rc::Rc::new(on_event);

        popup.on_accepted({
            let on_event = on_event.clone();
            move || {
                if let Some(popup) = popup_weak.upgrade() {
                    popup.hide().expect("failed to hide detached popup");
                }
                on_event(Event::Accepted);
            }
        });

        let popup_weak = popup.as_weak();
        popup.on_dismissed(move || {
            if let Some(popup) = popup_weak.upgrade() {
                popup.hide().expect("failed to hide detached popup");
            }
            on_event(Event::Dismissed);
        });
        let popup_weak = popup.as_weak();
        popup.on_minimize_requested(move || {
            if let Some(popup) = popup_weak.upgrade() { popup.window().set_minimized(true); }
        });
        let popup_weak = popup.as_weak();
        popup.on_maximize_requested(move || {
            if let Some(popup) = popup_weak.upgrade() { popup.window().set_maximized(true); }
        });

        Ok(Self { popup, custom_frame, custom_corner_radius })
    }

    pub fn show(&self, title: impl Into<slint::SharedString>, message: impl Into<slint::SharedString>) -> Result<(), slint::PlatformError> {
        self.popup.set_title_text(title.into());
        self.popup.set_message(message.into());
        self.popup.show()?;
        if let Some(config) = self.custom_frame {
            crate::ui_slint_win32_frame::install_now(&self.popup.window(), config);
        }
        if let Some(radius) = self.custom_corner_radius {
            crate::ui_slint_win32_frame::set_corner_radius(&self.popup.window(), radius);
        }
        Ok(())
    }

    pub fn set_topbar(&self, config: TopBarConfig) {
        self.popup.set_topbar_show_minimize(config.show_minimize); self.popup.set_topbar_show_maximize(config.show_maximize); self.popup.set_topbar_show_close(config.show_close);
        self.popup.set_topbar_height(config.height); self.popup.set_topbar_title_size(config.title_size);
    }

    /// Direct attention to the popup after its input-blocking caller was clicked.
    /// Windows may still decline a foreground request in situations where the OS
    /// focus policy disallows it, but this caller click normally grants it.
    pub fn restore_focus_and_flash(&self) {
        self.popup.window().set_minimized(false);
        let _ = self.popup.window().with_winit_window(|window| {
            let Ok(handle) = window.window_handle() else { return; };
            let RawWindowHandle::Win32(handle) = handle.as_raw() else { return; };
            let hwnd = handle.hwnd.get() as HWND;

            unsafe {
                let _ = SetForegroundWindow(hwnd);
                let _ = FlashWindowEx(&FLASHWINFO {
                    cbSize: size_of::<FLASHWINFO>() as u32,
                    hwnd,
                    dwFlags: FLASHW_CAPTION,
                    uCount: 3,
                    dwTimeout: 0,
                });
            }
        });
    }
}
