//! Slint adapter for the reusable tree view.
//!
//! This module has no filesystem knowledge and does not know which Slint
//! window hosts it. A host implements [`Port`] to connect its generated Slint
//! properties and callbacks to this view API.
//!
//! # Host contract
//! Implement `Port` to convert `Node` to the Slint `TreeNode` model and forward
//! `on_selected`, `on_expansion_toggled`, `on_activated`,
//! `on_drag_start_requested` and `on_context_menu_requested`. Call `bind` to
//! receive `Event`s and `set_nodes` to publish the caller-owned flattened tree.
//! Update nodes after selection/expansion; no-op callbacks drop input. Filesystem
//! traversal, icon loading and navigation are host responsibilities. UI sizing,
//! fields and options are documented in `ui/ui_tree.slint`.
//! Drag events only request a host action; the host owns native drag/drop,
//! context menus, payloads and operations. Coordinates are window-relative
//! logical pixels. `drop-viewport-hit` identifies the viewport, not a node.
//! Required: Rust `slint` crate and a host `Port`. The supplied visual requires
//! `ui/ui_tree.slint` and its `ui/ui_scroll_view.slint`; Rust helper is optional
//! if the caller binds Slint directly.

use slint::Image;

#[derive(Clone)]
pub struct Node {
    /// Section label above this row; empty means no section heading.
    pub section: String,
    /// Stable node ID returned in callbacks.
    pub id: String,
    /// Display text for the node.
    pub label: String,
    /// Indentation level; caller provides a flattened, ordered model.
    pub depth: i32,
    /// Controls expander direction; caller supplies visible descendants.
    pub expanded: bool,
    /// Caller-owned selected styling flag.
    pub selected: bool,
    /// Caller-loaded image; the view reserves an icon slot for every row.
    pub icon: Image,
}

/// User requests; the caller updates node data and performs navigation.
pub enum Event {
    /// Row clicked; string is node ID.
    Selected(String),
    /// Expander clicked; caller toggles and republishes the flattened model.
    ExpansionToggled(String),
    /// Row double-clicked; caller interprets activation.
    Activated(String),
    /// A node drag passed the movement threshold; host chooses the payload.
    DragStartRequested(String),
    /// Right press on a node, with window-relative logical coordinates.
    ContextMenuRequested { id: String, x: f32, y: f32 },
}

/// The small host-side bridge required by the view. Implement this next to
/// the generated window type, not inside this reusable module.
pub trait Port {
    fn replace_nodes(&self, nodes: Vec<Node>);
    fn on_selected(&self, callback: impl Fn(String) + 'static);
    fn on_expansion_toggled(&self, callback: impl Fn(String) + 'static);
    fn on_activated(&self, callback: impl Fn(String) + 'static);
    fn on_drag_start_requested(&self, callback: impl Fn(String) + 'static);
    fn on_context_menu_requested(&self, callback: impl Fn(String, f32, f32) + 'static);
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
    port.on_activated({ let on_event = on_event.clone(); move |id| on_event(Event::Activated(id)) });
    port.on_drag_start_requested({ let on_event = on_event.clone(); move |id| on_event(Event::DragStartRequested(id)) });
    port.on_context_menu_requested(move |id, x, y| on_event(Event::ContextMenuRequested { id, x, y }));
}
