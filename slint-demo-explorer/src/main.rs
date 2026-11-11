#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod explorer_model;
#[cfg(target_os = "windows")]
mod ui_slint_win32_frame;

use crate::explorer_model::ExplorerModel;
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

slint::include_modules!();

mod ui_tree;

const MAX_TREE_DEPTH: usize = 5;
const MAX_CHILDREN_PER_NODE: usize = 80;

// UiTree receives this final image value. A production caller can replace
// these demo assets with its own cache of Win32 shell icons, app icons, etc.
const DRIVE_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="2" y="5" width="20" height="14" rx="3" fill="#6f9bc8"/><rect x="5" y="8" width="14" height="3" rx="1" fill="#d9edff"/><circle cx="17" cy="15" r="1.4" fill="#233242"/></svg>"##;
const FOLDER_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2 6.5A2.5 2.5 0 0 1 4.5 4H10l2 2.5h7.5A2.5 2.5 0 0 1 22 9v8.5a2.5 2.5 0 0 1-2.5 2.5h-15A2.5 2.5 0 0 1 2 17.5Z" fill="#d4a94d"/><path d="M2.5 9h19v2h-19z" fill="#f0d27b"/></svg>"##;

#[derive(Clone)]
struct TreeIcons {
    drive: Image,
    folder: Image,
}

impl TreeIcons {
    fn load() -> Self {
        Self {
            drive: Image::load_from_svg_data(DRIVE_ICON_SVG).unwrap_or_default(),
            folder: Image::load_from_svg_data(FOLDER_ICON_SVG).unwrap_or_default(),
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

fn refresh(
    ui: &AppWindow,
    model: &ExplorerModel,
    selected: &str,
    expanded: &HashSet<String>,
    folder_cache: &mut HashMap<PathBuf, Vec<PathBuf>>,
    icons: &TreeIcons,
) {
    ui.set_location(model.location().display().to_string().into());
    ui_tree::set_nodes(ui, build_tree_nodes(
        model,
        expanded,
        folder_cache,
        icons,
    ));

    let entries = model
        .entries()
        .into_iter()
        .map(|entry| ListEntry {
            selected: entry.path.to_string_lossy() == selected,
            id: entry.path.to_string_lossy().into_owned().into(),
            name: entry.name.into(),
            type_name: entry.type_name.into(),
            modified: entry.modified.into(),
            size: entry.size.into(),
            is_folder: entry.is_folder,
        })
        .collect::<Vec<_>>();
    let count = entries.len();
    ui.set_list_entries(ModelRc::new(VecModel::from(entries)));
    ui.set_status(format!("{count} items · {}", model.location().display()).into());
}

fn main() -> Result<(), slint::PlatformError> {
    #[cfg(target_os = "windows")]
    ui_slint_win32_frame::select_alpha_winit_backend()?;

    let ui = AppWindow::new()?;
    let model = Rc::new(RefCell::new(ExplorerModel::new()));
    let selected = Rc::new(RefCell::new(String::new()));
    let expanded = Rc::new(RefCell::new(HashSet::new()));
    let folder_cache = Rc::new(RefCell::new(HashMap::new()));
    let icons = Rc::new(TreeIcons::load());
    expand_location_ancestors(&model.borrow(), &mut expanded.borrow_mut());
    refresh(
        &ui,
        &model.borrow(),
        &selected.borrow(),
        &expanded.borrow(),
        &mut folder_cache.borrow_mut(),
        &icons,
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
                refresh(&ui, &model.borrow(), &selected.borrow(), &expanded.borrow(), &mut folder_cache.borrow_mut(), &icons);
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
                refresh(&ui, &model.borrow(), &selected.borrow(), &expanded.borrow(), &mut folder_cache.borrow_mut(), &icons);
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
            match event {
                ui_tree::Event::Selected(id) | ui_tree::Event::Activated(id) => {
                    if model.borrow_mut().navigate(PathBuf::from(id)) {
                        selected.borrow_mut().clear();
                        expand_location_ancestors(&model.borrow(), &mut expanded.borrow_mut());
                    }
                }
                ui_tree::Event::ExpansionToggled(id) => {
                    let mut expanded = expanded.borrow_mut();
                    if !expanded.insert(id.clone()) {
                        expanded.remove(&id);
                    }
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                refresh(&ui, &model.borrow(), &selected.borrow(), &expanded.borrow(), &mut folder_cache.borrow_mut(), &icons);
            }
        }
    });
    ui.on_list_selected({
        let ui_weak = ui.as_weak();
        let model = model.clone();
        let selected = selected.clone();
        let expanded = expanded.clone();
        let folder_cache = folder_cache.clone();
        let icons = icons.clone();
        move |id: SharedString| {
            *selected.borrow_mut() = id.to_string();
            if let Some(ui) = ui_weak.upgrade() {
                refresh(&ui, &model.borrow(), &selected.borrow(), &expanded.borrow(), &mut folder_cache.borrow_mut(), &icons);
            }
        }
    });
    ui.on_list_activated({
        let ui_weak = ui.as_weak();
        let model = model.clone();
        let selected = selected.clone();
        let expanded = expanded.clone();
        let folder_cache = folder_cache.clone();
        let icons = icons.clone();
        move |id: SharedString| {
            if model.borrow_mut().navigate(PathBuf::from(id.to_string())) {
                selected.borrow_mut().clear();
                expand_location_ancestors(&model.borrow(), &mut expanded.borrow_mut());
            }
            if let Some(ui) = ui_weak.upgrade() {
                refresh(&ui, &model.borrow(), &selected.borrow(), &expanded.borrow(), &mut folder_cache.borrow_mut(), &icons);
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
