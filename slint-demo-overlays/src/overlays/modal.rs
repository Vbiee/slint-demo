//! Caller-facing modal contract; no application business logic belongs here.

use crate::AppWindow;

pub enum Event {
    Confirmed,
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
