//! Host-neutral adapter API for the generic Slint data list.
//!
//! # Host contract
//! - Required: Rust `slint` crate and host implementation of `Port`. To render
//!   with the supplied view, import `ui/ui_list.slint` and its mandatory
//!   `ui_scroll_view.slint`. The host generates and maps its own Slint types.
//! - Implement `Port` for a host adapter: map `Column`/`Row`/`Cell` into Slint
//!   models and update selected flags / one column width in the existing model.
//! - Forward each `on_*` callback to its matching Slint callback; a no-op method
//!   silently drops the event. Call `bind` to receive the corresponding `Event`.
//! - Host owns IDs, selection, focus, anchor, navigation and command execution.
//!   After selection requests, update row flags and the component's focused ID.
//! - `Selected` is currently not emitted by the Slint view. Selection modes,
//!   command codes, styling and gesture limitations are in `ui/ui_list.slint`.
//! - `set_columns`, `set_rows`, `set_selection` and `set_column_width` delegate
//!   to `Port`; they do not store state or supply application behavior.

use slint::{Color, Image};

#[derive(Clone)]
pub struct Column {
    /// Stable, unique ID within the columns model; used by resize callbacks.
    pub id: String,
    /// Header label, elided when it does not fit.
    pub title: String,
    /// Logical pixels: nonnegative is fixed, negative shares remaining width.
    pub width: f32,
    /// Minimum logical-pixel width for dragging and for an auto-width column.
    pub min_width: f32,
    /// 0 = left, 1 = center, 2 = right.
    pub alignment: i32,
    /// Enables the header resize handle; host must persist committed widths.
    pub resizable: bool,
    /// Primary field can start drag on a selected row; configure at most one.
    pub primary: bool,
    /// Cell text override; `None` uses selected/hover/normal theme color.
    pub text_color: Option<Color>,
    /// Cell background override; `None` leaves the row background visible.
    pub background_color: Option<Color>,
    /// Header text override; `None` uses the component header text color.
    pub header_text_color: Option<Color>,
    /// Header background override; `None` uses transparent header cells.
    pub header_background_color: Option<Color>,
}

#[derive(Clone)]
pub struct Cell {
    /// Text for this row and column, elided if it does not fit.
    pub text: String,
    /// Optional caller-owned icon rendered before this cell's text.
    pub icon: Option<Image>,
    /// Logical-pixel size of `icon` when present.
    pub icon_size: f32,
    /// Per-cell override, taking precedence over the column text color.
    pub text_color: Option<Color>,
    /// Per-cell override, taking precedence over the column background.
    pub background_color: Option<Color>,
}

#[derive(Clone)]
pub struct Row {
    /// Stable, unique row ID; selection and callbacks refer to this value.
    pub id: String,
    /// Values ordered by columns; missing cells render empty, extras ignored.
    pub cells: Vec<Cell>,
    /// Caller-owned selection flag to render, not maintained by the adapter.
    pub selected: bool,
}

/// Semantic requests from the view. The host owns all subsequent state changes.
pub enum Event {
    /// Compatibility callback; current Slint view does not emit it.
    Selected(String),
    /// `mode`: 0 replace, 1 toggle, 2 extend from caller's anchor.
    SelectionRequested { id: String, mode: i32 },
    /// Request to focus ID; independent from selection.
    FocusRequested(String),
    /// Double-click or Enter on ID; interpretation belongs to the host.
    Activated(String),
    /// Right-click ID and window-relative logical-pixel coordinates.
    ContextMenuRequested { id: String, x: f32, y: f32 },
    /// 1 rename, 2 delete, 3 copy, 4 cut, 5 paste, 6 select all, 7 clear.
    CommandRequested(i32),
    /// Column ID and committed width in logical pixels after resize release.
    ColumnResized { id: String, width: f32 },
    /// A selected row began a primary-field drag; host resolves selected IDs.
    DragStartRequested(String),
    /// A selection drag reached this row; intended for gesture feedback.
    SelectionDragged(String),
}

/// Host bridge to its generated Slint component. All methods must be provided;
/// forwarding a callback to a no-op discards that feature's requests.
pub trait Port {
    /// Replace the column model, mapping optional colors to has-* flags.
    fn replace_columns(&self, columns: Vec<Column>);
    /// Update the matching column's width; do not recreate delegates mid-drag.
    fn set_column_width(&self, id: &str, width: f32);
    /// Replace the row model, mapping cell icons/colors and their has-* flags.
    fn replace_rows(&self, rows: Vec<Row>);
    /// Set rendered row flags according to caller-owned selected IDs.
    fn set_selection(&self, ids: &[String]);
    /// Connect the corresponding Slint callback; see `Event::Selected`.
    fn on_selected(&self, callback: impl Fn(String) + 'static);
    /// Connect selection-requested; host interprets the mode and updates rows.
    fn on_selection_requested(&self, callback: impl Fn(String, i32) + 'static);
    /// Connect focus-requested and publish the host's focused ID separately.
    fn on_focus_requested(&self, callback: impl Fn(String) + 'static);
    /// Connect activated; callback argument is the row ID.
    fn on_activated(&self, callback: impl Fn(String) + 'static);
    /// Connect right-click ID and window-relative logical coordinates.
    fn on_context_menu_requested(&self, callback: impl Fn(String, f32, f32) + 'static);
    /// Connect the numeric command request; host executes or rejects it.
    fn on_command_requested(&self, callback: impl Fn(i32) + 'static);
    /// Connect column ID and committed logical-pixel width.
    fn on_column_resized(&self, callback: impl Fn(String, f32) + 'static);
    /// Forward primary-field drag request; this does not start native DnD.
    fn on_drag_start_requested(&self, callback: impl Fn(String) + 'static);
    /// Forward range-selection gesture feedback.
    fn on_selection_dragged(&self, callback: impl Fn(String) + 'static);
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
    port.on_column_resized({
        let on_event = on_event.clone();
        move |id, width| on_event(Event::ColumnResized { id, width })
    });
    port.on_drag_start_requested({
        let on_event = on_event.clone();
        move |id| on_event(Event::DragStartRequested(id))
    });
    port.on_selection_dragged(move |id| on_event(Event::SelectionDragged(id)));
}
