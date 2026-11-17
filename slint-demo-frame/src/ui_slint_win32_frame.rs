//! Reusable Win32 custom-window-frame bridge for Slint/Winit.
//!
//! We deliberately keep `WS_OVERLAPPEDWINDOW`: Windows/DWM therefore owns the
//! resize border, rounded corners and drop shadow.  `WM_NCCALCSIZE` only moves
//! the client area upward over the normal caption, and `WM_NCHITTEST` restores
//! resize/drag behaviour where Slint cannot express it.

use slint::winit_030::{
    winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    EventResult, WinitWindowAccessor,
};
use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    sync::{LazyLock, Mutex},
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::{
        Dwm::{
            DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_BORDER_COLOR,
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND,
        },
        Gdi::ScreenToClient,
    },
    UI::{
        Controls::MARGINS,
        WindowsAndMessaging::{
            CallWindowProcW, DefWindowProcW, GetClientRect, SetWindowLongPtrW, SetWindowPos,
            GWLP_WNDPROC, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT,
            HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOOWNERZORDER,
            SWP_NOSIZE, SWP_NOZORDER, WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST, WNDPROC,
        },
    },
};

/// Geometry for the app's custom title bar and Win32 resize hit targets.
/// Values are physical pixels because Win32 sends physical pointer coordinates.
#[derive(Debug, Clone, Copy)]
pub struct FrameConfig {
    pub resize_left: i32,
    pub resize_top: i32,
    pub resize_right: i32,
    pub resize_bottom: i32,
    pub title_bar_height: i32,
    pub caption_button_width: i32,
    /// `COLORREF` (`0x00bbggrr`), or `None` to use Windows' default border.
    pub border_color: Option<u32>,
}

impl Default for FrameConfig {
    fn default() -> Self {
        Self {
            resize_left: 8,
            resize_top: 8,
            resize_right: 8,
            resize_bottom: 8,
            title_bar_height: 64,
            caption_button_width: 156,
            border_color: Some(0x0021_1a_17), // RGB(0x17, 0x1a, 0x21)
        }
    }
}

#[derive(Clone, Copy)]
struct InstalledFrame {
    original_wnd_proc: isize,
    config: FrameConfig,
}

// One process-wide dispatcher is required by Win32, but every HWND has its own
// original procedure and geometry. This allows main windows and popup windows
// to use this module simultaneously.
static INSTALLED_FRAMES: LazyLock<Mutex<HashMap<isize, InstalledFrame>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Must be called once, before constructing any Slint component.
/// It requests an alpha-capable surface for smoothly transparent corners.
pub fn select_alpha_winit_backend() -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .with_winit_window_attributes_hook(|attributes| attributes.with_transparent(true))
        .select()
}

/// Installs custom non-client handling for one Slint window.
pub fn install(window: &slint::Window, config: FrameConfig) {
    // Winit creates its HWND only when the event loop is active. Install on the
    // first window event so `with_winit_window()` cannot race application startup.
    let installed = Rc::new(Cell::new(false));
    window.on_winit_window_event(move |window, _event| {
        if !installed.get() && install_native_hook(window, config) {
            installed.set(true);
        }
        EventResult::Propagate
    });
}

/// Installs immediately when the native HWND already exists (for example just
/// after `ComponentHandle::show()`). Returns false while the window is not
/// created yet; `install()` remains the deferred, normal setup API.
pub fn install_now(window: &slint::Window, config: FrameConfig) -> bool {
    install_native_hook(window, config)
}

fn install_native_hook(slint_window: &slint::Window, config: FrameConfig) -> bool {
    slint_window
        .with_winit_window(|window| {
            let Ok(handle) = window.window_handle() else {
                return false;
            };
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return false;
            };

            // The Slint callback runs on the Winit UI thread, so the HWND is valid here.
            unsafe { install_for_hwnd(handle.hwnd.get() as HWND, config) }
        })
        .unwrap_or(false)
}

/// Applies a Slint corner radius to native DWM behavior.
///
/// `0.0` preserves a 1px DWM seam for the native shadow. Any positive radius
/// uses an alpha edge, which avoids glass artifacts around rounded corners.
pub fn set_corner_radius(slint_window: &slint::Window, radius: f32) {
    let rounded = radius > 0.0;
    let _ = slint_window.with_winit_window(|window| {
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let preference = if rounded {
            DWMWCP_ROUND
        } else {
            DWMWCP_DONOTROUND
        };
        let _ = unsafe {
            DwmSetWindowAttribute(
                handle.hwnd.get() as HWND,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &preference as *const _ as *const _,
                size_of_val(&preference) as u32,
            )
        };
        // Square mode trades a 1px DWM composition seam for the native shadow.
        // Rounded mode uses a fully transparent alpha edge, so it must not have
        // an extended DWM glass frame behind it.
        let margins = MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: if rounded { 0 } else { 1 },
            cyBottomHeight: 0,
        };
        let _ = unsafe { DwmExtendFrameIntoClientArea(handle.hwnd.get() as HWND, &margins) };
        let _ = unsafe {
            SetWindowPos(
                handle.hwnd.get() as HWND,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER,
            )
        };
    });
}

unsafe fn install_for_hwnd(hwnd: HWND, config: FrameConfig) -> bool {
    let key = hwnd as isize;
    if let Some(frame) = INSTALLED_FRAMES
        .lock()
        .expect("window frame registry lock poisoned")
        .get_mut(&key)
    {
        frame.config = config;
        return true;
    }

    // Keep a 1px DWM composition seam so Windows retains its native shadow.
    let margins = MARGINS {
        cxLeftWidth: 0,
        cxRightWidth: 0,
        cyTopHeight: 1,
        cyBottomHeight: 0,
    };
    let _ = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };

    if let Some(border_color) = config.border_color {
        let _ = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR as u32,
                &border_color as *const _ as *const _,
                size_of_val(&border_color) as u32,
            )
        };
    }

    let original =
        unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, title_bar_wnd_proc as *const () as isize) };
    INSTALLED_FRAMES
        .lock()
        .expect("window frame registry lock poisoned")
        .insert(
            key,
            InstalledFrame {
                original_wnd_proc: original,
                config,
            },
        );
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER,
        )
    };
    true
}

unsafe extern "system" fn title_bar_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCDESTROY {
        let result = unsafe { call_original(hwnd, message, wparam, lparam) };
        INSTALLED_FRAMES
            .lock()
            .expect("window frame registry lock poisoned")
            .remove(&(hwnd as isize));
        return result;
    }

    match message {
        WM_NCCALCSIZE if wparam != 0 => {
            // Claim the full non-client area: top, left, right, and bottom are
            // all painted by Slint. WM_NCHITTEST below provides invisible resize
            // zones, so no visible system frame is required.
            return 0;
        }
        WM_NCHITTEST => {
            let native_result = unsafe { call_original(hwnd, message, wparam, lparam) };
            if native_result != HTCLIENT as isize {
                return native_result;
            }

            let mut point = POINT {
                x: (lparam as i16) as i32,
                y: ((lparam >> 16) as i16) as i32,
            };
            unsafe { ScreenToClient(hwnd, &mut point) };
            let mut rect = RECT::default();
            unsafe { GetClientRect(hwnd, &mut rect) };
            let config = frame_for(hwnd).map_or_else(FrameConfig::default, |frame| frame.config);
            let left = config.resize_left.max(0);
            let top = config.resize_top.max(0);
            let right = config.resize_right.max(0);
            let bottom = config.resize_bottom.max(0);

            // Let Windows retain all normal resize affordances.
            if point.y < top && point.x < left {
                return HTTOPLEFT as isize;
            }
            if point.y < top && point.x >= rect.right - right {
                return HTTOPRIGHT as isize;
            }
            if point.y >= rect.bottom - bottom && point.x < left {
                return HTBOTTOMLEFT as isize;
            }
            if point.y >= rect.bottom - bottom && point.x >= rect.right - right {
                return HTBOTTOMRIGHT as isize;
            }
            if point.y < top {
                return HTTOP as isize;
            }
            if point.y >= rect.bottom - bottom {
                return HTBOTTOM as isize;
            }
            if point.x < left {
                return HTLEFT as isize;
            }
            if point.x >= rect.right - right {
                return HTRIGHT as isize;
            }

            // Keep the right-hand buttons as client area, so Slint receives clicks.
            if point.y < config.title_bar_height.max(0)
                && point.x < rect.right - config.caption_button_width.max(0)
            {
                return HTCAPTION as isize;
            }
        }
        _ => {}
    }

    unsafe { call_original(hwnd, message, wparam, lparam) }
}

unsafe fn call_original(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(frame) = frame_for(hwnd) else {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    };
    let procedure: WNDPROC = unsafe { std::mem::transmute(frame.original_wnd_proc) };
    unsafe { CallWindowProcW(procedure, hwnd, message, wparam, lparam) }
}

fn frame_for(hwnd: HWND) -> Option<InstalledFrame> {
    INSTALLED_FRAMES
        .lock()
        .expect("window frame registry lock poisoned")
        .get(&(hwnd as isize))
        .copied()
}
