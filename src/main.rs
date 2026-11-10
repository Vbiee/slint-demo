#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
#[cfg(target_os = "windows")]
mod windows_chrome;

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
    windows_chrome::select_alpha_winit_backend()?;

    let ui = AppWindow::new()?;
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
        move |radius| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_corner_radius(radius);
                #[cfg(target_os = "windows")]
                windows_chrome::set_corner_radius(&ui.window(), radius);
            }
        }
    });

    #[cfg(target_os = "windows")]
    windows_chrome::install(&ui.window());

    // show() creates the native Winit window. The Windows event handler above
    // then extends the client area through the title bar while retaining DWM.
    ui.show()?;

    ui.run()
}
