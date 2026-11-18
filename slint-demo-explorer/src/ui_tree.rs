//! Slint adapter for the reusable tree view.
//!
//! This module has no filesystem knowledge and does not know which Slint
//! window hosts it. A host implements [`Port`] to connect its generated Slint
//! properties and callbacks to this view API.

use slint::Image;

#[derive(Clone)]
pub struct Node {
    pub section: String,
    pub id: String,
    pub label: String,
    pub depth: i32,
    pub expanded: bool,
    pub selected: bool,
    pub icon: Image,
}

pub enum Event {
    Selected(String),
    ExpansionToggled(String),
    Activated(String),
}

/// The small host-side bridge required by the view. Implement this next to
/// the generated window type, not inside this reusable module.
pub trait Port {
    fn replace_nodes(&self, nodes: Vec<Node>);
    fn on_selected(&self, callback: impl Fn(String) + 'static);
    fn on_expansion_toggled(&self, callback: impl Fn(String) + 'static);
    fn on_activated(&self, callback: impl Fn(String) + 'static);
}

/// Replace all rendered nodes through the host bridge.
pub fn set_nodes(port: &impl Port, nodes: Vec<Node>) {
    port.replace_nodes(nodes);
}

/// Forward host callbacks as view-level semantic events.
pub fn bind(port: &impl Port, on_event: impl Fn(Event) + 'static) {
    let on_event = std::rc::Rc::new(on_event);

    port.on_selected({
        let on_event = on_event.clone();
        move |id| on_event(Event::Selected(id))
    });
    port.on_expansion_toggled({
        let on_event = on_event.clone();
        move |id| on_event(Event::ExpansionToggled(id))
    });
    port.on_activated(move |id| on_event(Event::Activated(id)));
}
