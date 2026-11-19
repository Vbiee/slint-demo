#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod explorer_model;
#[cfg(target_os = "windows")]
mod ui_slint_win32_frame;

use crate::explorer_model::ExplorerModel;
use slint::{Color, ComponentHandle, Image, Model, ModelRc, SharedString, VecModel};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

slint::include_modules!();

mod ui_list;
mod ui_tree;

fn transparent_color() -> Color {
    Color::from_argb_u8(0, 0, 0, 0)
}

// AppWindow is the Explorer host. Keeping this generated-Slint bridge here
// means ui_tree and ui_list never depend on a particular window type.
impl ui_tree::Port for AppWindow {
    fn replace_nodes(&self, nodes: Vec<ui_tree::Node>) {
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
        self.set_tree_nodes(ModelRc::new(VecModel::from(nodes)));
    }

    fn on_selected(&self, callback: impl Fn(String) + 'static) {
        self.on_tree_selected(move |id| callback(id.to_string()));
    }

    fn on_expansion_toggled(&self, callback: impl Fn(String) + 'static) {
        self.on_tree_expansion_toggled(move |id| callback(id.to_string()));
    }

    fn on_activated(&self, callback: impl Fn(String) + 'static) {
        self.on_tree_activated(move |id| callback(id.to_string()));
    }
}

impl ui_list::Port for AppWindow {
    fn replace_columns(&self, columns: Vec<ui_list::Column>) {
        let columns = columns
            .into_iter()
            .map(|column| ListColumn {
                id: column.id.into(),
                title: column.title.into(),
                width: column.width,
                min_width: column.min_width,
                alignment: column.alignment,
                resizable: column.resizable,
                text_color: column.text_color.unwrap_or_else(transparent_color),
                has_text_color: column.text_color.is_some(),
                background_color: column.background_color.unwrap_or_else(transparent_color),
                has_background_color: column.background_color.is_some(),
                header_text_color: column.header_text_color.unwrap_or_else(transparent_color),
                has_header_text_color: column.header_text_color.is_some(),
                header_background_color: column.header_background_color.unwrap_or_else(transparent_color),
                has_header_background_color: column.header_background_color.is_some(),
            })
            .collect::<Vec<_>>();
        self.set_list_columns(ModelRc::new(VecModel::from(columns)));
    }

    fn set_column_width(&self, id: &str, width: f32) {
        let columns = self.get_list_columns();
        for row in 0..columns.row_count() {
            let Some(mut column) = columns.row_data(row) else {
                continue;
            };
            if column.id.as_str() == id {
                column.width = width;
                columns.set_row_data(row, column);
                break;
            }
        }
    }

    fn replace_rows(&self, rows: Vec<ui_list::Row>) {
        let rows = rows
            .into_iter()
            .map(|row| ListRow {
                id: row.id.into(),
                cells: ModelRc::new(VecModel::from(
                    row.cells
                        .into_iter()
                        .map(|cell| {
                            let has_icon = cell.icon.is_some();
                            ListCell {
                                text: cell.text.into(),
                                icon: cell.icon.unwrap_or_default(),
                                has_icon,
                                icon_size: cell.icon_size,
                                text_color: cell.text_color.unwrap_or_else(transparent_color),
                                has_text_color: cell.text_color.is_some(),
                                background_color: cell.background_color.unwrap_or_else(transparent_color),
                                has_background_color: cell.background_color.is_some(),
                            }
                        })
                        .collect::<Vec<_>>(),
                )),
                selected: row.selected,
            })
            .collect::<Vec<_>>();
        self.set_list_rows(ModelRc::new(VecModel::from(rows)));
    }

    fn set_selected(&self, id: &str) {
        // With UiList's virtual ListView, only visible delegates redraw.
        let rows = self.get_list_rows();
        for row in 0..rows.row_count() {
            let Some(mut item) = rows.row_data(row) else {
                continue;
            };
            let selected = item.id.as_str() == id;
            if item.selected != selected {
                item.selected = selected;
                rows.set_row_data(row, item);
            }
        }
    }

    fn on_selected(&self, callback: impl Fn(String) + 'static) {
        self.on_list_selected(move |id| callback(id.to_string()));
    }

    fn on_activated(&self, callback: impl Fn(String) + 'static) {
        self.on_list_activated(move |id| callback(id.to_string()));
    }

    fn on_column_resized(&self, callback: impl Fn(String, f32) + 'static) {
        self.on_list_column_resized(move |id, width| callback(id.to_string(), width));
    }
}

const MAX_TREE_DEPTH: usize = 5;
const MAX_CHILDREN_PER_NODE: usize = 80;

// UiTree receives this final image value. A production caller can replace
// these demo assets with its own cache of Win32 shell icons, app icons, etc.
const DRIVE_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="2" y="5" width="20" height="14" rx="3" fill="#6f9bc8"/><rect x="5" y="8" width="14" height="3" rx="1" fill="#d9edff"/><circle cx="17" cy="15" r="1.4" fill="#233242"/></svg>"##;
const FOLDER_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2 6.5A2.5 2.5 0 0 1 4.5 4H10l2 2.5h7.5A2.5 2.5 0 0 1 22 9v8.5a2.5 2.5 0 0 1-2.5 2.5h-15A2.5 2.5 0 0 1 2 17.5Z" fill="#d4a94d"/><path d="M2.5 9h19v2h-19z" fill="#f0d27b"/></svg>"##;
const FILE_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5 2.5h9l5 5v14H5Z" fill="#9aabc0"/><path d="M14 2.5v5h5" fill="#dce7f5"/><path d="M8 12h8M8 15h8M8 18h5" stroke="#526176" stroke-width="1.5" stroke-linecap="round"/></svg>"##;

#[derive(Clone)]
struct TreeIcons {
    drive: Image,
    folder: Image,
    file: Image,
}

impl TreeIcons {
    fn load() -> Self {
        Self {
            drive: Image::load_from_svg_data(DRIVE_ICON_SVG).unwrap_or_default(),
            folder: Image::load_from_svg_data(FOLDER_ICON_SVG).unwrap_or_default(),
            file: Image::load_from_svg_data(FILE_ICON_SVG).unwrap_or_default(),
        }
    }

    fn for_depth(&self, depth: usize) -> Image {
        if depth == 0 {
            self.drive.clone()
        } else {
            self.folder.clone()
        }
    }
}

fn path_id(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn label_for_path(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

fn folders_in(path: &Path, folder_cache: &mut HashMap<PathBuf, Vec<PathBuf>>) -> Vec<PathBuf> {
    folder_cache
        .entry(path.to_path_buf())
        .or_insert_with(|| {
            let mut folders = fs::read_dir(path)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    path.is_dir().then_some(path)
                })
                .collect::<Vec<_>>();
            folders.sort_by_cached_key(|path| label_for_path(path).to_lowercase());
            folders.truncate(MAX_CHILDREN_PER_NODE);
            folders
        })
        .clone()
}

fn append_tree_nodes(
    path: &Path,
    depth: usize,
    section: &str,
    current_location: &Path,
    expanded: &HashSet<String>,
    folder_cache: &mut HashMap<PathBuf, Vec<PathBuf>>,
    icons: &TreeIcons,
    nodes: &mut Vec<ui_tree::Node>,
) {
    let id = path_id(path);
    let is_expanded = expanded.contains(&id);
    // Reading a directory is synchronous. Only do it when that particular
    // branch is open; unopened folders optimistically show an expander.
    let children = if is_expanded && depth < MAX_TREE_DEPTH {
        folders_in(path, folder_cache)
    } else {
        Vec::new()
    };
    nodes.push(ui_tree::Node {
        section: section.into(),
        id: id.clone(),
        label: label_for_path(path),
        depth: depth as i32,
        expanded: is_expanded,
        selected: path == current_location,
        icon: icons.for_depth(depth),
    });

    if is_expanded {
        for child in children {
            append_tree_nodes(
                &child,
                depth + 1,
                "",
                current_location,
                expanded,
                folder_cache,
                icons,
                nodes,
            );
        }
    }
}

fn build_tree_nodes(
    model: &ExplorerModel,
    expanded: &HashSet<String>,
    folder_cache: &mut HashMap<PathBuf, Vec<PathBuf>>,
    icons: &TreeIcons,
) -> Vec<ui_tree::Node> {
    let mut nodes = Vec::new();
    // Match ui-explorer: one familiar entry point, then a separate drives
    // section. This keeps the tree navigable instead of a root-level dump.
    let folders_root = ExplorerModel::default_location();
    append_tree_nodes(
        &folders_root,
        0,
        "FOLDERS",
        model.location(),
        expanded,
        folder_cache,
        icons,
        &mut nodes,
    );

    for (index, root) in tree_roots(model.location()).into_iter().enumerate() {
        append_tree_nodes(
            &root,
            0,
            if index == 0 { "THIS PC" } else { "" },
            model.location(),
            expanded,
            folder_cache,
            icons,
            &mut nodes,
        );
    }
    nodes
}

#[cfg(target_os = "windows")]
fn tree_roots(_location: &Path) -> Vec<PathBuf> {
    // This is exactly the logical-drive enumeration that a Windows explorer
    // needs. It avoids probing A: through Z: (which can wake slow network
    // drives) and lets the caller decide which roots UiTree receives.
    let drive_bits = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
    (0..26)
        .filter(|bit| drive_bits & (1 << bit) != 0)
        .map(|bit| PathBuf::from(format!("{}:\\", (b'A' + bit as u8) as char)))
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn tree_roots(location: &Path) -> Vec<PathBuf> {
    let mut root = location.to_path_buf();
    while let Some(parent) = root.parent() {
        if parent == root {
            break;
        }
        root = parent.to_path_buf();
    }
    vec![root]
}

fn expand_location_ancestors(model: &ExplorerModel, expanded: &mut HashSet<String>) {
    // The selected node itself stays collapsed; only its parents open so the
    // selection is visible. This matches ui-explorer's OpenAncestors().
    for path in model.location().ancestors().skip(1) {
        expanded.insert(path_id(path));
    }
}

fn explorer_columns() -> Vec<ui_list::Column> {
    vec![
        ui_list::Column { id: "name".into(), title: "Name".into(), width: -1.0, min_width: 220.0, alignment: 0, resizable: true, text_color: None, background_color: None, header_text_color: None, header_background_color: None },
        ui_list::Column { id: "modified".into(), title: "Date modified".into(), width: 150.0, min_width: 110.0, alignment: 0, resizable: true, text_color: None, background_color: None, header_text_color: None, header_background_color: None },
        ui_list::Column { id: "type".into(), title: "Type".into(), width: 120.0, min_width: 90.0, alignment: 0, resizable: true, text_color: None, background_color: None, header_text_color: None, header_background_color: None },
        ui_list::Column { id: "size".into(), title: "Size".into(), width: 90.0, min_width: 70.0, alignment: 2, resizable: true, text_color: None, background_color: None, header_text_color: None, header_background_color: None },
    ]
}

fn refresh(
    ui: &AppWindow,
    model: &ExplorerModel,
    selected: &str,
    expanded: &HashSet<String>,
    folder_cache: &mut HashMap<PathBuf, Vec<PathBuf>>,
    icons: &TreeIcons,
    reload_list: bool,
) {
    ui.set_location(model.location().display().to_string().into());
    ui_tree::set_nodes(ui, build_tree_nodes(model, expanded, folder_cache, icons));

    if reload_list {
        let entries = model
            .entries()
            .into_iter()
            .map(|entry| ui_list::Row {
                selected: entry.path.to_string_lossy() == selected,
                id: entry.path.to_string_lossy().into_owned().into(),
                cells: vec![
                    ui_list::Cell {
                        text: entry.name,
                        icon: Some(if entry.is_folder { icons.folder.clone() } else { icons.file.clone() }),
                        icon_size: 18.0,
                        text_color: None,
                        background_color: None,
                    },
                    ui_list::Cell { text: entry.modified, icon: None, icon_size: 0.0, text_color: None, background_color: None },
                    ui_list::Cell { text: entry.type_name, icon: None, icon_size: 0.0, text_color: None, background_color: None },
                    ui_list::Cell { text: entry.size, icon: None, icon_size: 0.0, text_color: None, background_color: None },
                ],
            })
            .collect::<Vec<_>>();
        let count = entries.len();
        ui_list::set_rows(ui, entries);
        ui.set_list_status_text(format!("{count} items · {}", model.location().display()).into());
        ui.set_list_status_icon(icons.folder.clone());
        ui.set_list_has_status_icon(true);
        ui.set_list_status_icon_size(16.0);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    #[cfg(target_os = "windows")]
    ui_slint_win32_frame::select_alpha_winit_backend()?;

    let ui = AppWindow::new()?;
    let model = Rc::new(RefCell::new(ExplorerModel::new()));
    let selected = Rc::new(RefCell::new(String::new()));
    let expanded = Rc::new(RefCell::new(HashSet::new()));
    let columns = Rc::new(RefCell::new(explorer_columns()));
    let folder_cache = Rc::new(RefCell::new(HashMap::new()));
    let icons = Rc::new(TreeIcons::load());
    ui_list::set_columns(&ui, columns.borrow().clone());
    expand_location_ancestors(&model.borrow(), &mut expanded.borrow_mut());
    refresh(
        &ui,
        &model.borrow(),
        &selected.borrow(),
        &expanded.borrow(),
        &mut folder_cache.borrow_mut(),
        &icons,
        true,
    );

    ui.on_navigate_up({
        let ui_weak = ui.as_weak();
        let model = model.clone();
        let selected = selected.clone();
        let expanded = expanded.clone();
        let folder_cache = folder_cache.clone();
        let icons = icons.clone();
        move || {
            model.borrow_mut().navigate_up();
            selected.borrow_mut().clear();
            expand_location_ancestors(&model.borrow(), &mut expanded.borrow_mut());
            if let Some(ui) = ui_weak.upgrade() {
                refresh(
                    &ui,
                    &model.borrow(),
                    &selected.borrow(),
                    &expanded.borrow(),
                    &mut folder_cache.borrow_mut(),
                    &icons,
                    true,
                );
            }
        }
    });
    ui.on_navigate_to({
        let ui_weak = ui.as_weak();
        let model = model.clone();
        let selected = selected.clone();
        let expanded = expanded.clone();
        let folder_cache = folder_cache.clone();
        let icons = icons.clone();
        move |path: SharedString| {
            if model.borrow_mut().navigate(PathBuf::from(path.to_string())) {
                selected.borrow_mut().clear();
                expand_location_ancestors(&model.borrow(), &mut expanded.borrow_mut());
            }
            if let Some(ui) = ui_weak.upgrade() {
                refresh(
                    &ui,
                    &model.borrow(),
                    &selected.borrow(),
                    &expanded.borrow(),
                    &mut folder_cache.borrow_mut(),
                    &icons,
                    true,
                );
            }
        }
    });
    ui_tree::bind(&ui, {
        let ui_weak = ui.as_weak();
        let model = model.clone();
        let selected = selected.clone();
        let expanded = expanded.clone();
        let folder_cache = folder_cache.clone();
        let icons = icons.clone();
        move |event| {
            let reload_list = match event {
                ui_tree::Event::Selected(id) | ui_tree::Event::Activated(id) => {
                    let navigated = model.borrow_mut().navigate(PathBuf::from(id));
                    if navigated {
                        selected.borrow_mut().clear();
                        expand_location_ancestors(&model.borrow(), &mut expanded.borrow_mut());
                    }
                    navigated
                }
                ui_tree::Event::ExpansionToggled(id) => {
                    let mut expanded = expanded.borrow_mut();
                    if !expanded.insert(id.clone()) {
                        expanded.remove(&id);
                    }
                    false
                }
            };
            if let Some(ui) = ui_weak.upgrade() {
                refresh(
                    &ui,
                    &model.borrow(),
                    &selected.borrow(),
                    &expanded.borrow(),
                    &mut folder_cache.borrow_mut(),
                    &icons,
                    reload_list,
                );
            }
        }
    });
    ui_list::bind(&ui, {
        let ui_weak = ui.as_weak();
        let model = model.clone();
        let selected = selected.clone();
        let expanded = expanded.clone();
        let folder_cache = folder_cache.clone();
        let icons = icons.clone();
        let columns = columns.clone();
        move |event| match event {
            ui_list::Event::Selected(id) => {
                *selected.borrow_mut() = id;
                if let Some(ui) = ui_weak.upgrade() {
                    ui_list::set_selected(&ui, &selected.borrow());
                }
            }
            ui_list::Event::Activated(id) => {
                if model.borrow_mut().navigate(PathBuf::from(id)) {
                    selected.borrow_mut().clear();
                    expand_location_ancestors(&model.borrow(), &mut expanded.borrow_mut());
                    if let Some(ui) = ui_weak.upgrade() {
                        refresh(
                            &ui,
                            &model.borrow(),
                            &selected.borrow(),
                            &expanded.borrow(),
                            &mut folder_cache.borrow_mut(),
                            &icons,
                            true,
                        );
                    }
                }
            }
            ui_list::Event::ColumnResized { id, width } => {
                let mut columns = columns.borrow_mut();
                if let Some(column) = columns.iter_mut().find(|column| column.id == id) {
                    column.width = width;
                }
                if let Some(ui) = ui_weak.upgrade() {
                    ui_list::set_column_width(&ui, &id, width);
                }
            }
        }
    });

    #[cfg(target_os = "windows")]
    ui_slint_win32_frame::install(
        &ui.window(),
        ui_slint_win32_frame::FrameConfig {
            title_bar_height: 44,
            caption_button_width: 220,
            ..Default::default()
        },
    );
    ui.show()?;
    // Explorer does not expose a corner-radius control, so select the square
    // mode explicitly: it retains the native DWM shadow.
    ui_slint_win32_frame::set_corner_radius(&ui.window(), 0.0);
    ui.run()
}
