#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
#[cfg(target_os = "windows")]
mod ui_slint_win32_frame;
#[cfg(target_os = "windows")]
mod ui_slint_win32_control;

use crate::core::TaskStore;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

slint::include_modules!();

fn refresh(ui: &AppWindow, store: &TaskStore) {
    let rows = store
        .tasks()
        .iter()
        .map(|task| TaskItem {
            title: task.title.clone().into(),
            done: task.done,
        })
        .collect::<Vec<_>>();
    ui.set_tasks(ModelRc::new(VecModel::from(rows)));
    ui.set_status(
        format!(
            "{} tasks · {} completed",
            store.tasks().len(),
            store.completed_count()
        )
        .into(),
    );
}

fn main() -> Result<(), slint::PlatformError> {
    #[cfg(target_os = "windows")]
    ui_slint_win32_control::UiSlintWin32Control::select_backend()?;

    let ui = AppWindow::new()?;
    #[cfg(target_os = "windows")]
    let mut window_control = ui_slint_win32_control::UiSlintWin32Control::new(Default::default());
    let store = Rc::new(RefCell::new(TaskStore::demo()));
    refresh(&ui, &store.borrow());

    ui.on_add_task({
        let ui_weak = ui.as_weak();
        let store = store.clone();
        move |title: SharedString| {
            store.borrow_mut().add(title.to_string());
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_draft("".into());
                refresh(&ui, &store.borrow());
            }
        }
    });
    ui.on_toggle_task({
        let ui_weak = ui.as_weak();
        let store = store.clone();
        move |index| {
            store.borrow_mut().toggle(index as usize);
            if let Some(ui) = ui_weak.upgrade() {
                refresh(&ui, &store.borrow());
            }
        }
    });
    ui.on_remove_task({
        let ui_weak = ui.as_weak();
        let store = store.clone();
        move |index| {
            store.borrow_mut().remove(index as usize);
            if let Some(ui) = ui_weak.upgrade() {
                refresh(&ui, &store.borrow());
            }
        }
    });
    ui.on_set_window_corner_radius({
        let ui_weak = ui.as_weak();
        #[cfg(target_os = "windows")]
        let mut window_control = window_control;
        move |radius| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_corner_radius(radius);
                #[cfg(target_os = "windows")]
                window_control.set_corner_radius(&ui.window(), radius);
            }
        }
    });

    #[cfg(target_os = "windows")]
    ui.on_window_minimize_requested({
        let ui_weak = ui.as_weak();
        let window_control = window_control;
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                window_control.minimize(&ui.window());
            }
        }
    });
    #[cfg(target_os = "windows")]
    ui.on_window_maximize_requested({
        let ui_weak = ui.as_weak();
        let window_control = window_control;
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                window_control.toggle_maximized(&ui.window());
            }
        }
    });
    ui.on_window_close_requested({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let _ = ui.hide();
            }
        }
    });

    #[cfg(target_os = "windows")]
    window_control.install(&ui.window());

    // show() creates the native Winit window. The Windows event handler above
    // then extends the client area through the title bar while retaining DWM.
    ui.show()?;

    #[cfg(target_os = "windows")]
    window_control.after_show(&ui.window());

    ui.run()
}
