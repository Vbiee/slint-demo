#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod app_window_context_menu_port;
mod overlays;
#[path = "../../slint-demo-frame/src/ui_slint_win32_frame.rs"]
mod ui_slint_win32_frame;

use overlays::{context_menu, modal, popup};
use slint::ComponentHandle;

fn menu_items() -> Vec<context_menu::Item> {
    vec![
        context_menu::Item { id: "open".into(), label: "Open".into(), shortcut: "Enter".into(), enabled: true, ..Default::default() },
        context_menu::Item { id: "rename".into(), label: "Rename…".into(), shortcut: "F2".into(), enabled: true, ..Default::default() },
        context_menu::Item { id: "separator-edit".into(), separator: true, ..Default::default() },
        context_menu::Item { id: "preview".into(), label: "Preview pane".into(), checked: true, enabled: true, ..Default::default() },
        context_menu::Item { id: "copy".into(), label: "Copy".into(), shortcut: "Ctrl+C".into(), enabled: false, ..Default::default() },
        context_menu::Item { id: "delete".into(), label: "Delete".into(), shortcut: "Del".into(), enabled: true, danger: true, ..Default::default() },
    ]
}

fn main() -> Result<(), slint::PlatformError> {
    // One application-wide setup; individual windows opt into a custom frame below.
    ui_slint_win32_frame::select_alpha_winit_backend()?;
    let ui = AppWindow::new()?;
    let detached_popup = std::rc::Rc::new(popup::PopupWindowManager::new(
        popup::PopupOptions {
            frame: popup::FrameStyle::CustomWin32(
                ui_slint_win32_frame::FrameConfig {
                    title_bar_height: 46,
                    caption_button_width: 56,
                    ..Default::default()
                },
            ),
            // Zero keeps the 1px DWM seam, which lets Windows render its
            // native shadow. Positive custom radii trade that shadow away.
            corner_radius: 0.0,
        },
        {
        let ui_weak = ui.as_weak();
        move |event| {
            if let Some(ui) = ui_weak.upgrade() {
                let status = match event {
                    popup::Event::Accepted => "Detached popup applied its action.",
                    popup::Event::Dismissed => "Detached popup was closed.",
                };
                ui.set_native_popup_locking(false);
                ui.set_status(status.into());
            }
        }
    },
    )?);
    let context_menu = std::rc::Rc::new(context_menu::ContextMenuManager::new(
        app_window_context_menu_port::AppWindowContextMenuPort::new(&ui),
        context_menu::ContextMenuOptions::default(),
        {
        let ui_weak = ui.as_weak();
        move |event| {
            if let Some(ui) = ui_weak.upgrade() {
                match event {
                    context_menu::Event::ItemTriggered(id) => {
                        ui.set_status(format!("Context-menu command: {id}").into());
                    }
                    context_menu::Event::Dismissed => {
                        ui.set_status("Context menu dismissed.".into());
                    }
                }
            }
        }
    },
    )?);

    ui.on_request_modal({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                modal::set_open(&ui, true);
            }
        }
    });
    ui.on_request_detached_popup({
        let popup = detached_popup.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_native_popup_locking(ui.get_lock_caller());
            popup.set_topbar(popup::TopBarConfig { show_minimize: ui.get_topbar_minimize(), show_maximize: ui.get_topbar_maximize(), show_close: ui.get_topbar_close(), height: ui.get_topbar_height() as f32, title_size: ui.get_topbar_font_size() as f32 });
            popup
                .show(ui.get_topbar_title(), "This UI belongs to a separate Slint Window. The caller only asks PopupWindowManager to show it.")
                .expect("failed to show detached popup");
        }
    });
    ui.on_request_in_app_context_menu({
        let context_menu = context_menu.clone();
        let ui_weak = ui.as_weak();
        move |x, y| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let _ = context_menu.show(ui.get_context_menu_native(), x, y, menu_items());
        }
    });
    ui.on_caller_blocked({
        let popup = detached_popup.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if ui.get_native_popup_locking() {
                popup.restore_focus_and_flash();
                ui.set_status("The locked popup was focused and flashed.".into());
            }
        }
    });
    ui.on_request_context_menu({
        let context_menu = context_menu.clone();
        let ui_weak = ui.as_weak();
        move |x, y| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let _ = context_menu.show(ui.get_context_menu_native(), x, y, menu_items());
        }
    });

    modal::bind(&ui, {
        let ui_weak = ui.as_weak();
        move |event| {
            if let Some(ui) = ui_weak.upgrade() {
                match event {
                    modal::Event::Confirmed => ui.set_status("Modal confirmed by caller.".into()),
                    modal::Event::Cancelled => ui.set_status("Modal cancelled by caller.".into()),
                }
                modal::set_open(&ui, false);
            }
        }
    });
    let _detached_popup = detached_popup;
    let _context_menu = context_menu;
    ui.run()
}
