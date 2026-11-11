//! Slint adapter for the reusable tree view.
//!
//! This module has no filesystem knowledge. A caller supplies nodes and
//! receives semantic events; `ui_tree.slint` remains only the visual/input
//! implementation.

use crate::{AppWindow, TreeNode};
use slint::{Image, ModelRc, SharedString, VecModel};

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

/// Replace the rendered tree while keeping Slint-specific model types here.
pub fn set_nodes(ui: &AppWindow, nodes: Vec<Node>) {
    let nodes = nodes
        .into_iter()
        .map(|node| TreeNode {
            section: node.section.into(),
            id: node.id.into(),
            label: node.label.into(),
            depth: node.depth,
            expanded: node.expanded,
            selected: node.selected,
            icon: node.icon,
        })
        .collect::<Vec<_>>();
    ui.set_tree_nodes(ModelRc::new(VecModel::from(nodes)));
}

/// Forward UI callbacks as caller-level events.
pub fn bind(ui: &AppWindow, on_event: impl Fn(Event) + 'static) {
    let on_event = std::rc::Rc::new(on_event);

    ui.on_tree_selected({
        let on_event = on_event.clone();
        move |id: SharedString| on_event(Event::Selected(id.to_string()))
    });
    ui.on_tree_expansion_toggled({
        let on_event = on_event.clone();
        move |id: SharedString| on_event(Event::ExpansionToggled(id.to_string()))
    });
    ui.on_tree_activated(move |id: SharedString| on_event(Event::Activated(id.to_string())));
}
