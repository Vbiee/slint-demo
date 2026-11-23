//! Demo-host binding for the reusable context-menu module.
//!
//! This is deliberately outside `overlays::context_menu`: it is the only
//! place that knows the generated Slint component is named `AppWindow` and
//! which properties that particular host exposes.

use crate::{AppWindow, ContextMenuItem};
use crate::overlays::context_menu::ContextMenuPort;
use slint::{ComponentHandle, ModelRc, PhysicalPosition};

#[derive(Clone)]
pub struct AppWindowContextMenuPort {
    ui: slint::Weak<AppWindow>,
}

impl AppWindowContextMenuPort {
    pub fn new(ui: &AppWindow) -> Self {
        Self { ui: ui.as_weak() }
    }
}

impl ContextMenuPort for AppWindowContextMenuPort {
    fn set_context_menu_items(&self, items: ModelRc<ContextMenuItem>) {
        if let Some(ui) = self.ui.upgrade() { ui.set_context_menu_items(items); }
    }

    fn set_context_menu_anchor(&self, x: f32, y: f32) {
        if let Some(ui) = self.ui.upgrade() {
            ui.set_context_menu_anchor_x(x);
            ui.set_context_menu_anchor_y(y);
        }
    }

    fn set_context_menu_open(&self, open: bool) {
        if let Some(ui) = self.ui.upgrade() { ui.set_context_menu_open(open); }
    }

    fn on_context_menu_item_triggered(&self, callback: impl Fn(String) + 'static) {
        if let Some(ui) = self.ui.upgrade() {
            ui.on_context_menu_item_triggered(move |id| callback(id.to_string()));
        }
    }

    fn on_context_menu_dismissed(&self, callback: impl Fn() + 'static) {
        if let Some(ui) = self.ui.upgrade() { ui.on_context_menu_dismissed(callback); }
    }

    fn on_context_menu_replacement_requested(&self, callback: impl Fn(f32, f32) + 'static) {
        if let Some(ui) = self.ui.upgrade() {
            ui.on_context_menu_replacement_requested(move |x, y| callback(x, y));
        }
    }

    fn context_menu_screen_position(&self, x: f32, y: f32) -> PhysicalPosition {
        let Some(ui) = self.ui.upgrade() else { return PhysicalPosition::new(0, 0); };
        let position = ui.window().position();
        let scale = ui.window().scale_factor();
        PhysicalPosition::new(
            position.x + (x * scale).round() as i32,
            position.y + (y * scale).round() as i32,
        )
    }

    fn context_menu_contains_screen_position(&self, point: PhysicalPosition) -> bool {
        let Some(ui) = self.ui.upgrade() else { return false; };
        let position = ui.window().position();
        let size = ui.window().size();
        point.x >= position.x && point.y >= position.y
            && point.x < position.x + size.width as i32
            && point.y < position.y + size.height as i32
    }
}
