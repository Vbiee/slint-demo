//! Demo-host adapter for an in-app modal; no business logic belongs here.
//!
//! Required: this demo's generated `crate::AppWindow` properties/callbacks;
//! this Rust adapter is therefore host-specific. The standalone
//! `ui/ui_modal.slint` is independent and does not require this adapter.
//! `set_open` updates caller-owned visibility. `bind` forwards confirmation and
//! cancellation requests; the caller must close the modal and perform actions.

use crate::AppWindow;

pub enum Event {
    /// User clicked the confirm button.
    Confirmed,
    /// User clicked cancel or the backdrop.
    Cancelled,
}

pub fn set_open(ui: &AppWindow, open: bool) {
    ui.set_modal_open(open);
}

pub fn bind(ui: &AppWindow, on_event: impl Fn(Event) + 'static) {
    let on_event = std::rc::Rc::new(on_event);
    ui.on_modal_confirmed({
        let on_event = on_event.clone();
        move || on_event(Event::Confirmed)
    });
    ui.on_modal_cancelled(move || on_event(Event::Cancelled));
}
