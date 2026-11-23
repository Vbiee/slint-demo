//! Host-neutral adapter API for the generic Slint data list.

use slint::{Color, Image};

#[derive(Clone)]
pub struct Column {
    pub id: String,
    pub title: String,
    /// A positive logical-pixel width is fixed; -1.0 means auto-stretch.
    pub width: f32,
    pub min_width: f32,
    /// 0 = left, 1 = center, 2 = right.
    pub alignment: i32,
    pub resizable: bool,
    pub text_color: Option<Color>,
    pub background_color: Option<Color>,
    pub header_text_color: Option<Color>,
    pub header_background_color: Option<Color>,
}

#[derive(Clone)]
pub struct Cell {
    pub text: String,
    /// Optional caller-owned icon rendered before this cell's text.
    pub icon: Option<Image>,
    /// Logical-pixel size of `icon` when present.
    pub icon_size: f32,
    pub text_color: Option<Color>,
    pub background_color: Option<Color>,
}

#[derive(Clone)]
pub struct Row {
    pub id: String,
    pub cells: Vec<Cell>,
    pub selected: bool,
}

pub enum Event {
    Selected(String),
    SelectionRequested { id: String, mode: i32 },
    FocusRequested(String),
    Activated(String),
    ContextMenuRequested { id: String, x: f32, y: f32 },
    CommandRequested(i32),
    ColumnResized { id: String, width: f32 },
}

pub trait Port {
    fn replace_columns(&self, columns: Vec<Column>);
    fn set_column_width(&self, id: &str, width: f32);
    fn replace_rows(&self, rows: Vec<Row>);
    fn set_selection(&self, ids: &[String]);
    fn on_selected(&self, callback: impl Fn(String) + 'static);
    fn on_selection_requested(&self, callback: impl Fn(String, i32) + 'static);
    fn on_focus_requested(&self, callback: impl Fn(String) + 'static);
    fn on_activated(&self, callback: impl Fn(String) + 'static);
    fn on_context_menu_requested(&self, callback: impl Fn(String, f32, f32) + 'static);
    fn on_command_requested(&self, callback: impl Fn(i32) + 'static);
    fn on_column_resized(&self, callback: impl Fn(String, f32) + 'static);
}

pub fn set_columns(port: &impl Port, columns: Vec<Column>) {
    port.replace_columns(columns);
}

/// Update one width in place so an active header drag is not recreated.
pub fn set_column_width(port: &impl Port, id: &str, width: f32) {
    port.set_column_width(id, width);
}

pub fn set_rows(port: &impl Port, rows: Vec<Row>) {
    port.replace_rows(rows);
}

pub fn set_selection(port: &impl Port, ids: &[String]) {
    port.set_selection(ids);
}

pub fn bind(port: &impl Port, on_event: impl Fn(Event) + 'static) {
    let on_event = std::rc::Rc::new(on_event);
    port.on_selected({
        let on_event = on_event.clone();
        move |id| on_event(Event::Selected(id))
    });
    port.on_selection_requested({
        let on_event = on_event.clone();
        move |id, mode| on_event(Event::SelectionRequested { id, mode })
    });
    port.on_focus_requested({
        let on_event = on_event.clone();
        move |id| on_event(Event::FocusRequested(id))
    });
    port.on_activated({
        let on_event = on_event.clone();
        move |id| on_event(Event::Activated(id))
    });
    port.on_context_menu_requested({
        let on_event = on_event.clone();
        move |id, x, y| on_event(Event::ContextMenuRequested { id, x, y })
    });
    port.on_command_requested({
        let on_event = on_event.clone();
        move |command| on_event(Event::CommandRequested(command))
    });
    port.on_column_resized(move |id, width| on_event(Event::ColumnResized { id, width }));
}
