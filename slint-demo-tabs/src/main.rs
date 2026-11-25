#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../../slint-demo-frame/src/ui_slint_win32_frame.rs"]
mod ui_slint_win32_frame;
#[path = "../../slint-demo-frame/src/ui_slint_win32_control.rs"]
mod ui_slint_win32_control;
// Reuse the host-neutral Explorer adapters and filesystem-only model. The
// tab demo supplies its own AppWindow bridge below; it does not import the
// Explorer demo's AppWindow or main loop.
#[path = "../../slint-demo-explorer/src/explorer_model.rs"]
mod explorer_model;
#[path = "../../slint-demo-explorer/src/ui_tree.rs"]
mod ui_tree;
#[path = "../../slint-demo-explorer/src/ui_list.rs"]
mod ui_list;
mod ui_tabs;

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    path::{Path, PathBuf},
    rc::Rc,
};

use slint::{Color, ComponentHandle, Image, Model, ModelRc, SharedString, VecModel};
use ui_slint_win32_control::{Options as WindowChromeOptions, UiSlintWin32Control};

slint::include_modules!();

const DOCUMENT_ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path fill="#75b8ff" d="M3 1h6l4 4v10H3z"/><path fill="#17212d" d="M9 1v4h4"/><path fill="#dcecff" d="M5 8h6v1H5zm0 3h6v1H5z"/></svg>"##;

fn document_icon() -> Image {
    Image::load_from_svg_data(DOCUMENT_ICON_SVG.as_bytes()).unwrap_or_default()
}

fn tab_item(id: &str, title: &str, closable: bool, text_color: Option<Color>) -> ui_tabs::Tab {
    ui_tabs::Tab { id: id.into(), title: title.into(), icon: Some(document_icon()),
        icon_size: 16., text_color, closable, enabled: true }
}

fn tabs() -> Vec<ui_tabs::Tab> {
    vec![
        tab_item("explorer", "Explorer", false, None),
        tab_item("welcome", "Welcome", true, None),
        tab_item("inspector", "Inspector", true, Some(Color::from_rgb_u8(0xa9, 0xd6, 0xff))),
        tab_item("preview", "Preview", true, None),
    ]
}

// Demo-specific bridge: the reusable tab state never imports AppWindow or
// generated Slint types; each host maps its groups to its own properties.
fn push_models(ui: &AppWindow, workspace: &ui_tabs::Workspace) {
    let convert = |group: &ui_tabs::Group| ModelRc::new(VecModel::from(group.tabs().iter().map(|tab| TabItem {
        id: tab.id.clone().into(), title: tab.title.clone().into(),
        icon: tab.icon.clone().unwrap_or_default(), has_icon: tab.icon.is_some(), icon_size: tab.icon_size,
        text_color: tab.text_color.unwrap_or_else(transparent_color), has_text_color: tab.text_color.is_some(),
        closable: tab.closable, enabled: tab.enabled,
    }).collect::<Vec<_>>()));
    let primary = workspace.group("primary").expect("demo primary group");
    let secondary = workspace.group("secondary").expect("demo secondary group");
    ui.set_primary_tabs(convert(primary));
    ui.set_secondary_tabs(convert(secondary));
    ui.set_primary_active_tab_id(primary.active_id().unwrap_or("").into());
    ui.set_secondary_active_tab_id(secondary.active_id().unwrap_or("").into());
}

fn transparent_color() -> Color { Color::from_argb_u8(0, 0, 0, 0) }

// AppWindow is the only adapter that knows generated Slint property names.
// `ui_tree` and `ui_list` stay host/window independent.
impl ui_tree::Port for AppWindow {
    fn replace_nodes(&self, nodes: Vec<ui_tree::Node>) {
        self.set_explorer_tree_nodes(ModelRc::new(VecModel::from(nodes.into_iter().map(|node| TreeNode {
            section: node.section.into(), id: node.id.into(), label: node.label.into(),
            depth: node.depth, expanded: node.expanded, selected: node.selected, icon: node.icon,
        }).collect::<Vec<_>>())));
    }
    fn on_selected(&self, callback: impl Fn(String) + 'static) {
        self.on_explorer_tree_selected(move |id| callback(id.to_string()));
    }
    fn on_expansion_toggled(&self, callback: impl Fn(String) + 'static) {
        self.on_explorer_tree_expansion_toggled(move |id| callback(id.to_string()));
    }
    fn on_activated(&self, callback: impl Fn(String) + 'static) { self.on_explorer_tree_activated(move |id| callback(id.to_string())); }
}

impl ui_list::Port for AppWindow {
    fn replace_columns(&self, columns: Vec<ui_list::Column>) {
        self.set_explorer_list_columns(ModelRc::new(VecModel::from(columns.into_iter().map(|column| ListColumn {
            id: column.id.into(), title: column.title.into(), width: column.width,
            min_width: column.min_width, alignment: column.alignment, resizable: column.resizable,
            primary: column.primary,
            text_color: column.text_color.unwrap_or_else(transparent_color), has_text_color: column.text_color.is_some(),
            background_color: column.background_color.unwrap_or_else(transparent_color), has_background_color: column.background_color.is_some(),
            header_text_color: column.header_text_color.unwrap_or_else(transparent_color), has_header_text_color: column.header_text_color.is_some(),
            header_background_color: column.header_background_color.unwrap_or_else(transparent_color), has_header_background_color: column.header_background_color.is_some(),
        }).collect::<Vec<_>>())));
    }
    fn set_column_width(&self, id: &str, width: f32) {
        let columns = self.get_explorer_list_columns();
        for index in 0..columns.row_count() {
            if let Some(mut column) = columns.row_data(index) {
                if column.id.as_str() == id { column.width = width; columns.set_row_data(index, column); break; }
            }
        }
    }
    fn replace_rows(&self, rows: Vec<ui_list::Row>) {
        self.set_explorer_list_rows(ModelRc::new(VecModel::from(rows.into_iter().map(|row| ListRow {
            id: row.id.into(),
            cells: ModelRc::new(VecModel::from(row.cells.into_iter().map(|cell| {
                let has_icon = cell.icon.is_some();
                ListCell { text: cell.text.into(), icon: cell.icon.unwrap_or_default(), has_icon, icon_size: cell.icon_size,
                    text_color: cell.text_color.unwrap_or_else(transparent_color), has_text_color: cell.text_color.is_some(),
                    background_color: cell.background_color.unwrap_or_else(transparent_color), has_background_color: cell.background_color.is_some() }
            }).collect::<Vec<_>>())), selected: row.selected,
        }).collect::<Vec<_>>())));
    }
    fn set_selection(&self, ids: &[String]) {
        let selected_ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let rows = self.get_explorer_list_rows();
        for index in 0..rows.row_count() {
            if let Some(mut row) = rows.row_data(index) {
                let selected = selected_ids.contains(row.id.as_str());
                if row.selected != selected { row.selected = selected; rows.set_row_data(index, row); }
            }
        }
    }
    fn on_selected(&self, callback: impl Fn(String) + 'static) { self.on_explorer_list_selected(move |id| callback(id.to_string())); }
    fn on_selection_requested(&self, callback: impl Fn(String, i32) + 'static) { self.on_explorer_list_selection_requested(move |id, mode| callback(id.to_string(), mode)); }
    fn on_drag_start_requested(&self, callback: impl Fn(String) + 'static) { self.on_explorer_list_drag_start_requested(move |id| callback(id.to_string())); }
    fn on_selection_dragged(&self, callback: impl Fn(String) + 'static) { self.on_explorer_list_selection_dragged(move |id| callback(id.to_string())); }
    fn on_focus_requested(&self, callback: impl Fn(String) + 'static) { self.on_explorer_list_focus_requested(move |id| callback(id.to_string())); }
    fn on_activated(&self, callback: impl Fn(String) + 'static) { self.on_explorer_list_activated(move |id| callback(id.to_string())); }
    fn on_context_menu_requested(&self, callback: impl Fn(String, f32, f32) + 'static) { self.on_explorer_list_context_menu_requested(move |id, x, y| callback(id.to_string(), x, y)); }
    fn on_command_requested(&self, callback: impl Fn(i32) + 'static) { self.on_explorer_list_command_requested(callback); }
    fn on_column_resized(&self, callback: impl Fn(String, f32) + 'static) { self.on_explorer_list_column_resized(move |id, width| callback(id.to_string(), width)); }
}

const FOLDER_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2 6.5A2.5 2.5 0 0 1 4.5 4H10l2 2.5h7.5A2.5 2.5 0 0 1 22 9v8.5a2.5 2.5 0 0 1-2.5 2.5h-15A2.5 2.5 0 0 1 2 17.5Z" fill="#d4a94d"/></svg>"##;
const FILE_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5 2.5h9l5 5v14H5Z" fill="#9aabc0"/><path d="M14 2.5v5h5" fill="#dce7f5"/></svg>"##;

#[derive(Clone)]
struct ExplorerIcons { folder: Image, file: Image }
impl ExplorerIcons {
    fn load() -> Self { Self { folder: Image::load_from_svg_data(FOLDER_ICON_SVG).unwrap_or_default(), file: Image::load_from_svg_data(FILE_ICON_SVG).unwrap_or_default() } }
}

fn explorer_columns() -> Vec<ui_list::Column> {
    vec![
        ui_list::Column { id: "name".into(), title: "Name".into(), width: -1., min_width: 220., alignment: 0, resizable: true, primary: true, text_color: None, background_color: None, header_text_color: None, header_background_color: None },
        ui_list::Column { id: "modified".into(), title: "Date modified".into(), width: 150., min_width: 110., alignment: 0, resizable: true, primary: false, text_color: None, background_color: None, header_text_color: None, header_background_color: None },
        ui_list::Column { id: "type".into(), title: "Type".into(), width: 120., min_width: 90., alignment: 0, resizable: true, primary: false, text_color: None, background_color: None, header_text_color: None, header_background_color: None },
        ui_list::Column { id: "size".into(), title: "Size".into(), width: 90., min_width: 70., alignment: 2, resizable: true, primary: false, text_color: None, background_color: None, header_text_color: None, header_background_color: None },
    ]
}

fn path_id(path: &Path) -> String { path.to_string_lossy().into_owned() }
fn path_label(path: &Path) -> String { path.file_name().map(|x| x.to_string_lossy().into_owned()).filter(|x| !x.is_empty()).unwrap_or_else(|| path.display().to_string()) }

fn explorer_tree(model: &explorer_model::ExplorerModel, expanded: &HashSet<String>, icons: &ExplorerIcons) -> Vec<ui_tree::Node> {
    let root = explorer_model::ExplorerModel::default_location();
    let mut nodes = Vec::new();
    append_tree(&root, 0, "FOLDERS", model.location(), expanded, icons, &mut nodes);
    nodes
}

fn append_tree(path: &Path, depth: usize, section: &str, current: &Path, expanded: &HashSet<String>, icons: &ExplorerIcons, nodes: &mut Vec<ui_tree::Node>) {
    let id = path_id(path);
    let is_expanded = expanded.contains(&id);
    nodes.push(ui_tree::Node { section: section.into(), id: id.clone(), label: path_label(path), depth: depth as i32, expanded: is_expanded, selected: path == current, icon: icons.folder.clone() });
    if is_expanded && depth < 5 {
        let mut children = std::fs::read_dir(path).into_iter().flatten().flatten().filter_map(|entry| { let child = entry.path(); child.is_dir().then_some(child) }).collect::<Vec<_>>();
        children.sort_by_cached_key(|child| path_label(child).to_lowercase());
        children.truncate(80);
        for child in children { append_tree(&child, depth + 1, "", current, expanded, icons, nodes); }
    }
}

#[derive(Default)]
struct ExplorerSelection {
    ids: HashSet<String>,
    focused_id: String,
    anchor_id: String,
}

impl ExplorerSelection {
    fn clear(&mut self) { *self = Self::default(); }
    fn replace(&mut self, id: String) {
        self.ids.clear();
        self.ids.insert(id.clone());
        self.anchor_id = id.clone();
        self.focused_id = id;
    }
    fn request(&mut self, id: String, mode: i32, ordered_ids: &[String]) {
        if !ordered_ids.contains(&id) { return; }
        match mode {
            1 => {
                if !self.ids.insert(id.clone()) { self.ids.remove(&id); }
                self.anchor_id = id.clone();
                self.focused_id = id;
            }
            2 => {
                let anchor = ordered_ids.iter().position(|item| item == &self.anchor_id);
                let target = ordered_ids.iter().position(|item| item == &id);
                if let (Some(a), Some(b)) = (anchor, target) {
                    self.ids = ordered_ids[a.min(b)..=a.max(b)].iter().cloned().collect();
                    self.focused_id = id;
                } else { self.replace(id); }
            }
            _ => self.replace(id),
        }
    }
}

fn list_ids(ui: &AppWindow) -> Vec<String> {
    let rows = ui.get_explorer_list_rows();
    (0..rows.row_count()).filter_map(|index| rows.row_data(index).map(|row| row.id.to_string())).collect()
}

fn publish_selection(ui: &AppWindow, selection: &ExplorerSelection) {
    let ids = selection.ids.iter().cloned().collect::<Vec<_>>();
    ui_list::set_selection(ui, &ids);
    ui.set_explorer_list_focused_id(selection.focused_id.clone().into());
    ui.set_explorer_operation_status(format!("{} selected", ids.len()).into());
}

fn refresh_explorer(ui: &AppWindow, model: &explorer_model::ExplorerModel, selected: &ExplorerSelection, expanded: &HashSet<String>, icons: &ExplorerIcons, reload_rows: bool) {
    ui.set_explorer_location(model.location().display().to_string().into());
    ui.set_explorer_list_focused_id(selected.focused_id.clone().into());
    ui.set_explorer_operation_status(format!("{} selected", selected.ids.len()).into());
    ui_tree::set_nodes(ui, explorer_tree(model, expanded, icons));
    if reload_rows {
        let rows = model.entries().into_iter().map(|entry| ui_list::Row {
            id: entry.path.to_string_lossy().into_owned(), selected: selected.ids.contains(entry.path.to_string_lossy().as_ref()),
            cells: vec![
                ui_list::Cell { text: entry.name, icon: Some(if entry.is_folder { icons.folder.clone() } else { icons.file.clone() }), icon_size: 18., text_color: None, background_color: None },
                ui_list::Cell { text: entry.modified, icon: None, icon_size: 0., text_color: None, background_color: None },
                ui_list::Cell { text: entry.type_name, icon: None, icon_size: 0., text_color: None, background_color: None },
                ui_list::Cell { text: entry.size, icon: None, icon_size: 0., text_color: None, background_color: None },
            ],
        }).collect::<Vec<_>>();
        ui_list::set_rows(ui, rows);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    UiSlintWin32Control::select_backend()?;
    let ui = AppWindow::new()?;
    let window_control = UiSlintWin32Control::new(WindowChromeOptions {
        frame: ui_slint_win32_frame::FrameConfig {
            title_bar_height: 46,
            caption_button_width: 140,
            ..Default::default()
        },
        corner_radius: 0.0,
    });
    let mut initial_workspace = ui_tabs::Workspace::new(&["primary", "secondary"]).expect("valid demo groups");
    for tab in tabs() { initial_workspace.add("primary", tab, false); }
    initial_workspace.activate("primary", "welcome");
    let workspace = Rc::new(RefCell::new(initial_workspace));
    let next_tab_number = Rc::new(Cell::new(1_u32));
    push_models(&ui, &workspace.borrow());

    // This is a real hosted document, not a mock: filesystem state belongs
    // to this host and is projected into the reusable tree/list contracts.
    let explorer_model = Rc::new(RefCell::new(explorer_model::ExplorerModel::new()));
    let explorer_selected = Rc::new(RefCell::new(ExplorerSelection::default()));
    let explorer_expanded = Rc::new(RefCell::new(HashSet::<String>::new()));
    let explorer_columns_state = Rc::new(RefCell::new(explorer_columns()));
    let explorer_icons = Rc::new(ExplorerIcons::load());
    for ancestor in explorer_model.borrow().location().ancestors().skip(1) {
        explorer_expanded.borrow_mut().insert(path_id(ancestor));
    }
    ui_list::set_columns(&ui, explorer_columns_state.borrow().clone());
    refresh_explorer(&ui, &explorer_model.borrow(), &explorer_selected.borrow(), &explorer_expanded.borrow(), &explorer_icons, true);

    ui.on_explorer_navigate_up({
        let ui_weak = ui.as_weak(); let model = explorer_model.clone(); let selected = explorer_selected.clone();
        let expanded = explorer_expanded.clone(); let icons = explorer_icons.clone();
        move || {
            if model.borrow_mut().navigate_up() { selected.borrow_mut().clear(); }
            if let Some(ui) = ui_weak.upgrade() { refresh_explorer(&ui, &model.borrow(), &selected.borrow(), &expanded.borrow(), &icons, true); }
        }
    });
    ui.on_explorer_navigate_to({
        let ui_weak = ui.as_weak(); let model = explorer_model.clone(); let selected = explorer_selected.clone();
        let expanded = explorer_expanded.clone(); let icons = explorer_icons.clone();
        move |destination| {
            if model.borrow_mut().navigate(PathBuf::from(destination.to_string())) { selected.borrow_mut().clear(); }
            if let Some(ui) = ui_weak.upgrade() { refresh_explorer(&ui, &model.borrow(), &selected.borrow(), &expanded.borrow(), &icons, true); }
        }
    });
    ui_tree::bind(&ui, {
        let ui_weak = ui.as_weak(); let model = explorer_model.clone(); let selected = explorer_selected.clone();
        let expanded = explorer_expanded.clone(); let icons = explorer_icons.clone();
        move |event| {
            let reload = match event {
                ui_tree::Event::Selected(id) => model.borrow_mut().navigate(PathBuf::from(id)),
                ui_tree::Event::ExpansionToggled(id) => { let mut set = expanded.borrow_mut(); if !set.insert(id.clone()) { set.remove(&id); } false }
                ui_tree::Event::Activated(id) => model.borrow_mut().navigate(PathBuf::from(id)),
            };
            if reload { selected.borrow_mut().clear(); }
            if let Some(ui) = ui_weak.upgrade() { refresh_explorer(&ui, &model.borrow(), &selected.borrow(), &expanded.borrow(), &icons, reload); }
        }
    });
    ui_list::bind(&ui, {
        let ui_weak = ui.as_weak(); let model = explorer_model.clone(); let selected = explorer_selected.clone();
        let expanded = explorer_expanded.clone(); let icons = explorer_icons.clone(); let columns = explorer_columns_state.clone();
        move |event| match event {
            ui_list::Event::Selected(id) => if let Some(ui) = ui_weak.upgrade() {
                selected.borrow_mut().replace(id);
                publish_selection(&ui, &selected.borrow());
            },
            ui_list::Event::SelectionRequested { id, mode } => if let Some(ui) = ui_weak.upgrade() {
                selected.borrow_mut().request(id, mode, &list_ids(&ui));
                publish_selection(&ui, &selected.borrow());
            },
            ui_list::Event::DragStartRequested(id) => if let Some(ui) = ui_weak.upgrade() {
                ui.set_explorer_operation_status(format!("Drag start: {} selected ({})", selected.borrow().ids.len(), path_label(Path::new(&id))).into());
            },
            ui_list::Event::SelectionDragged(id) => if let Some(ui) = ui_weak.upgrade() {
                ui.set_explorer_operation_status(format!("Selecting: {} selected ({})", selected.borrow().ids.len(), path_label(Path::new(&id))).into());
            },
            ui_list::Event::FocusRequested(id) => {
                selected.borrow_mut().focused_id = id.clone();
                if let Some(ui) = ui_weak.upgrade() { ui.set_explorer_list_focused_id(id.into()); }
            }
            ui_list::Event::ContextMenuRequested { id, x, y } => if let Some(ui) = ui_weak.upgrade() {
                if !selected.borrow().ids.contains(&id) { selected.borrow_mut().replace(id); }
                publish_selection(&ui, &selected.borrow());
                ui.set_explorer_menu_x(x);
                ui.set_explorer_menu_y(y);
                ui.set_explorer_menu_open(true);
            },
            ui_list::Event::CommandRequested(command) => if let Some(ui) = ui_weak.upgrade() {
                match command {
                    6 => {
                        let ids = list_ids(&ui);
                        let mut state = selected.borrow_mut();
                        state.ids = ids.iter().cloned().collect();
                        if state.focused_id.is_empty() { state.focused_id = ids.first().cloned().unwrap_or_default(); }
                        state.anchor_id = state.focused_id.clone();
                    }
                    7 => selected.borrow_mut().clear(),
                    _ => {}
                }
                publish_selection(&ui, &selected.borrow());
                if (1..=5).contains(&command) {
                    let action = ["", "Rename", "Delete", "Copy", "Cut", "Paste"][command as usize];
                    ui.set_explorer_operation_status(format!("{action}: not implemented").into());
                }
            },
            ui_list::Event::Activated(id) => {
                if model.borrow_mut().navigate(PathBuf::from(id)) { selected.borrow_mut().clear();
                    if let Some(ui) = ui_weak.upgrade() { refresh_explorer(&ui, &model.borrow(), &selected.borrow(), &expanded.borrow(), &icons, true); }
                }
            }
            ui_list::Event::ColumnResized { id, width } => {
                if let Some(column) = columns.borrow_mut().iter_mut().find(|column| column.id == id) { column.width = width; }
                if let Some(ui) = ui_weak.upgrade() { ui_list::set_column_width(&ui, &id, width); }
            }
        }
    });

    ui.on_tab_selected({
        let ui_weak = ui.as_weak();
        let workspace = workspace.clone();
        move |group: SharedString, id: SharedString| {
            if !workspace.borrow_mut().activate(group.as_str(), id.as_str()) { return; }
            if let Some(ui) = ui_weak.upgrade() {
                push_models(&ui, &workspace.borrow());
                ui.set_status(format!("Active tab in {group} group: {id}").into());
            }
        }
    });
    ui.on_tab_close_requested({
        let ui_weak = ui.as_weak();
        let workspace = workspace.clone();
        move |group: SharedString, id: SharedString| {
            if !workspace.borrow_mut().close(group.as_str(), id.as_str()) { return; }
            if let Some(ui) = ui_weak.upgrade() {
                push_models(&ui, &workspace.borrow());
                ui.set_status(format!("Closed tab {id} in {group} group").into());
            }
        }
    });
    ui.on_split_right_requested({
        let ui_weak = ui.as_weak();
        let workspace = workspace.clone();
        move || {
            let mut state = workspace.borrow_mut();
            if state.group("secondary").is_some_and(|group| group.tabs().is_empty()) {
                // Explorer's content is currently mounted in the primary slot.
                // Other document IDs can safely move to the second demo pane.
                let candidate = state.group("primary").and_then(|group| group.tabs().iter()
                    .rev().find(|tab| tab.id != "explorer" && tab.enabled).map(|tab| tab.id.clone()));
                if let Some(id) = candidate { state.move_tab("primary", "secondary", &id); }
            }
            if let Some(ui) = ui_weak.upgrade() {
                push_models(&ui, &state);
                ui.set_split_enabled(true);
                ui.set_status("Created a second document group; drag the divider to resize it.".into());
            }
        }
    });
    ui.on_new_tab_requested({
        let ui_weak = ui.as_weak();
        let workspace = workspace.clone();
        let next_tab_number = next_tab_number.clone();
        move |title: SharedString| {
            let title = title.trim().to_owned();
            let Some(ui) = ui_weak.upgrade() else { return; };
            if title.is_empty() {
                ui.set_status("Enter a title before creating a tab.".into());
                return;
            }
            let number = next_tab_number.get();
            let id = format!("document-{number}");
            let mut state = workspace.borrow_mut();
            let target = state.active_group().to_owned();
            let tab = ui_tabs::Tab { id, title: title.clone(), icon: None, icon_size: 16.,
                text_color: None, closable: true, enabled: true };
            if !state.add(&target, tab, true) {
                ui.set_status("Could not create tab: duplicate ID or unknown group.".into());
                return;
            }
            next_tab_number.set(number + 1);
            push_models(&ui, &state);
            ui.set_new_tab_title("".into());
            ui.set_status(format!("Created tab {title} in {target} group").into());
        }
    });
    ui.on_split_ratio_changed({
        let ui_weak = ui.as_weak();
        move |ratio| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_status(format!("Document group split: {:.0}% / {:.0}%", ratio * 100., (1. - ratio) * 100.).into());
            }
        }
    });
    ui.on_window_minimize_requested({
        let ui_weak = ui.as_weak();
        let window_control = window_control;
        move || if let Some(ui) = ui_weak.upgrade() { window_control.minimize(&ui.window()); }
    });
    ui.on_window_maximize_requested({
        let ui_weak = ui.as_weak();
        let window_control = window_control;
        move || if let Some(ui) = ui_weak.upgrade() { window_control.toggle_maximized(&ui.window()); }
    });
    ui.on_window_close_requested({
        let ui_weak = ui.as_weak();
        move || if let Some(ui) = ui_weak.upgrade() { let _ = ui.hide(); }
    });

    window_control.install(&ui.window());
    ui.show()?;
    window_control.after_show(&ui.window());
    ui.run()
}

#[cfg(test)]
mod tests {
    use super::ExplorerSelection;

    #[test]
    fn selection_modes_follow_display_order_and_preserve_anchor() {
        let rows = ["a", "b", "c", "d"].map(String::from);
        let mut selection = ExplorerSelection::default();
        selection.request("b".into(), 0, &rows);
        selection.request("d".into(), 2, &rows);
        assert_eq!(selection.ids.len(), 3);
        assert!(selection.ids.contains("b") && selection.ids.contains("c") && selection.ids.contains("d"));
        assert_eq!(selection.anchor_id, "b");
        selection.request("a".into(), 1, &rows);
        assert!(selection.ids.contains("a"));
        assert_eq!(selection.anchor_id, "a");
        selection.request("c".into(), 2, &rows);
        assert_eq!(selection.ids.len(), 3);
        assert!(!selection.ids.contains("d"));
        selection.request("b".into(), 1, &rows);
        assert!(!selection.ids.contains("b"));
        selection.clear();
        assert!(selection.ids.is_empty() && selection.focused_id.is_empty() && selection.anchor_id.is_empty());
    }

    #[test]
    fn list_primary_drag_and_selection_drag_are_distinct() {
        use super::*;
        use slint::platform::{PointerEventButton, WindowEvent};
        use slint::{LogicalPosition, LogicalSize};

        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        ui.window().set_size(LogicalSize::new(900., 580.));
        ui.set_primary_active_tab_id("explorer".into());
        ui_list::set_columns(&ui, explorer_columns());
        let rows = (0..4).map(|index| ui_list::Row {
            id: format!("row-{index}"), selected: index < 2, cells: vec![],
        }).collect();
        ui_list::set_rows(&ui, rows);
        let drags = Rc::new(RefCell::new(Vec::<String>::new()));
        let selections = Rc::new(RefCell::new(Vec::<(String, i32)>::new()));
        let range = Rc::new(RefCell::new(Vec::<String>::new()));
        ui.on_explorer_list_drag_start_requested({ let events = drags.clone(); move |id| events.borrow_mut().push(id.to_string()) });
        ui.on_explorer_list_selection_requested({ let events = selections.clone(); move |id, mode| events.borrow_mut().push((id.to_string(), mode)) });
        ui.on_explorer_list_selection_dragged({ let events = range.clone(); move |id| events.borrow_mut().push(id.to_string()) });
        ui.show().unwrap();
        let pointer = |x, y| LogicalPosition::new(x, y);
        let press = |x, y| ui.window().dispatch_event(WindowEvent::PointerPressed { position: pointer(x, y), button: PointerEventButton::Left });
        let motion = |x, y| ui.window().dispatch_event(WindowEvent::PointerMoved { position: pointer(x, y) });
        let release = |x, y| ui.window().dispatch_event(WindowEvent::PointerReleased { position: pointer(x, y), button: PointerEventButton::Left });

        // Name is primary: selected row drag must not emit selection/click.
        motion(300., 205.); press(300., 205.); motion(310., 205.); release(310., 205.);
        assert_eq!(&*drags.borrow(), &["row-0"]);
        assert!(selections.borrow().is_empty());
        // Date modified is non-primary: drag extends the range, never starts drag.
        motion(620., 205.); press(620., 205.); motion(620., 245.); release(620., 245.);
        assert_eq!(drags.borrow().len(), 1);
        assert!(selections.borrow().iter().any(|(id, mode)| id == "row-1" && *mode == 2));
        assert!(range.borrow().iter().any(|id| id == "row-1"));
        // A click in a primary field still requests selection once.
        selections.borrow_mut().clear();
        motion(300., 205.); press(300., 205.); release(300., 205.);
        assert_eq!(&*selections.borrow(), &[("row-0".into(), 0)]);
        // An unselected primary row cannot start an item drag.
        selections.borrow_mut().clear();
        motion(300., 277.); press(300., 277.); motion(310., 277.); release(310., 277.);
        assert_eq!(drags.borrow().len(), 1);
        // Small movement is still a click, not a drag.
        selections.borrow_mut().clear();
        motion(300., 205.); press(300., 205.); motion(302., 205.); release(302., 205.);
        assert_eq!(drags.borrow().len(), 1);
        assert_eq!(&*selections.borrow(), &[("row-0".into(), 0)]);
        // Primary is caller-configured, not tied to the first column.
        let mut columns = explorer_columns();
        columns[0].primary = false;
        columns[1].primary = true;
        ui_list::set_columns(&ui, columns);
        selections.borrow_mut().clear();
        motion(620., 205.); press(620., 205.); motion(630., 205.); release(630., 205.);
        assert_eq!(&*drags.borrow(), &["row-0", "row-0"]);
        assert!(selections.borrow().is_empty());
    }
}
