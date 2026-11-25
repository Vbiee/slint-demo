//! Host-independent tab state for `ui/ui_tabs.slint`.
//!
//! The host creates stable, unique group/tab IDs and owns document content and
//! its lifetime. This module stores tab presentation, membership and active IDs;
//! it does not know AppWindow or instantiate a document. The host converts
//! `Tab` to Slint's `TabItem`, publishes each group's tabs/active ID, and routes
//! `tab-selected`/`tab-close-requested` to `activate`/`close`. A rejected action
//! returns false; only a successful close authorizes the host to dispose content.
//! Splitting, placement, scroll and sizing remain host/Slint workspace policy.
//! Required: Rust `slint` crate (`Image`, `Color`). Optional for display:
//! `ui/ui_tabs.slint` with its required `ui/ui_scroll_view.slint`; the host
//! implements the mapping to its generated Slint types. Group IDs are fixed at
//! construction; this helper has no API to add or remove groups afterwards.

use slint::{Color, Image};

#[derive(Clone, Debug)]
pub struct Tab {
    /// Nonempty stable ID, unique across all groups in this workspace.
    pub id: String,
    /// Caller-supplied header text.
    pub title: String,
    /// Optional caller-loaded icon; `None` produces no icon.
    pub icon: Option<Image>,
    /// Square icon size in logical pixels when icon is present.
    pub icon_size: f32,
    /// Header text override; `None` uses UiTabGroup's theme.
    pub text_color: Option<Color>,
    /// Whether the host permits `close`; also controls the close button.
    pub closable: bool,
    /// Whether the tab can be activated/moved and receive header input.
    pub enabled: bool,
}

pub struct Group {
    /// Caller-defined group ID, fixed when the workspace is constructed.
    pub id: String,
    tabs: Vec<Tab>,
    active_id: Option<String>,
}

impl Group {
    /// Display order of the group's tab descriptors.
    pub fn tabs(&self) -> &[Tab] { &self.tabs }
    /// Active tab ID, or None when the group contains no enabled active tab.
    pub fn active_id(&self) -> Option<&str> { self.active_id.as_deref() }
}

pub struct Workspace {
    groups: Vec<Group>,
    active_group: String,
}

impl Workspace {
    /// Group IDs must be nonempty and unique; the first group starts active.
    pub fn new(group_ids: &[&str]) -> Option<Self> {
        if group_ids.is_empty() || group_ids.iter().any(|id| id.is_empty())
            || group_ids.iter().enumerate().any(|(i, id)| group_ids[..i].contains(id)) {
            return None;
        }
        Some(Self {
            groups: group_ids.iter().map(|id| Group { id: (*id).into(), tabs: Vec::new(), active_id: None }).collect(),
            active_group: group_ids[0].into(),
        })
    }

    /// Look up a group by ID, or return None; read tabs and active ID via Group.
    pub fn group(&self, id: &str) -> Option<&Group> { self.groups.iter().find(|group| group.id == id) }
    /// Last activated target group; it can be empty if all groups have no tab.
    pub fn active_group(&self) -> &str { &self.active_group }

    /// Add to an existing group. Returns false for unknown group, empty ID, or
    /// duplicate workspace-wide ID. First enabled tab becomes group-active even
    /// if `activate` is false; true also makes its group active when enabled.
    pub fn add(&mut self, group_id: &str, tab: Tab, activate: bool) -> bool {
        if tab.id.is_empty() || self.groups.iter().any(|group| group.tabs.iter().any(|item| item.id == tab.id)) {
            return false;
        }
        let Some(group) = self.groups.iter_mut().find(|group| group.id == group_id) else { return false; };
        if tab.enabled && (activate || group.active_id.is_none()) { group.active_id = Some(tab.id.clone()); }
        group.tabs.push(tab);
        if activate && group.active_id.as_deref() == group.tabs.last().map(|item| item.id.as_str()) {
            self.active_group = group_id.into();
        }
        true
    }

    /// Select an enabled tab and make its group active; false if absent/disabled.
    pub fn activate(&mut self, group_id: &str, tab_id: &str) -> bool {
        let Some(group) = self.groups.iter_mut().find(|group| group.id == group_id) else { return false; };
        if !group.tabs.iter().any(|tab| tab.id == tab_id && tab.enabled) { return false; }
        group.active_id = Some(tab_id.into());
        self.active_group = group_id.into();
        true
    }

    /// An active tab is replaced by the nearest enabled neighbor; closing an
    /// inactive tab preserves activation. Returns false for unknown group/ID or
    /// non-closable tab. If the active group becomes empty, another nonempty
    /// group's active ID is preferred; caller disposes content after success.
    pub fn close(&mut self, group_id: &str, tab_id: &str) -> bool {
        let Some(group) = self.groups.iter_mut().find(|group| group.id == group_id) else { return false; };
        let Some(index) = group.tabs.iter().position(|tab| tab.id == tab_id && tab.closable) else { return false; };
        group.tabs.remove(index);
        if group.active_id.as_deref() == Some(tab_id) {
            group.active_id = group.tabs[index..].iter().chain(group.tabs[..index].iter().rev())
                .find(|tab| tab.enabled).map(|tab| tab.id.clone());
        }
        if group.active_id.is_none() && self.active_group == group_id {
            if let Some(other) = self.groups.iter().find(|other| other.active_id.is_some()) {
                self.active_group = other.id.clone();
            }
        }
        true
    }

    /// Move without closing content, append to target and activate it there.
    /// False for unknown/same groups, absent tab, or disabled tab. The host
    /// must reattach or preserve the corresponding document UI itself.
    pub fn move_tab(&mut self, from: &str, to: &str, tab_id: &str) -> bool {
        let Some(source) = self.groups.iter().position(|group| group.id == from) else { return false; };
        let Some(target) = self.groups.iter().position(|group| group.id == to) else { return false; };
        if source == target { return false; }
        let Some(index) = self.groups[source].tabs.iter().position(|tab| tab.id == tab_id && tab.enabled) else { return false; };
        let tab = self.groups[source].tabs.remove(index);
        if self.groups[source].active_id.as_deref() == Some(tab_id) {
            let tabs = &self.groups[source].tabs;
            self.groups[source].active_id = tabs[index..].iter().chain(tabs[..index].iter().rev())
                .find(|item| item.enabled).map(|item| item.id.clone());
        }
        self.groups[target].active_id = Some(tab.id.clone());
        self.groups[target].tabs.push(tab);
        self.active_group = to.into();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(id: &str, closable: bool, enabled: bool) -> Tab {
        Tab { id: id.into(), title: id.into(), icon: None, icon_size: 16., text_color: None, closable, enabled }
    }

    #[test]
    fn activation_closing_and_unique_ids() {
        assert!(Workspace::new(&["a", "a"]).is_none());
        let mut state = Workspace::new(&["a", "b"]).unwrap();
        assert!(state.add("a", tab("one", false, true), true));
        assert!(state.add("a", tab("two", true, true), false));
        assert!(!state.add("b", tab("one", true, true), true));
        assert!(!state.close("a", "one"));
        assert!(state.close("a", "two"));
        assert_eq!(state.group("a").unwrap().active_id(), Some("one"));
        assert!(!state.activate("a", "missing"));
    }

    #[test]
    fn closing_active_skips_disabled_and_moving_keeps_other_selection() {
        let mut state = Workspace::new(&["a", "b"]).unwrap();
        state.add("a", tab("one", true, true), true);
        state.add("a", tab("disabled", true, false), false);
        state.add("a", tab("three", true, true), false);
        assert!(state.close("a", "one"));
        assert_eq!(state.group("a").unwrap().active_id(), Some("three"));
        state.add("a", tab("four", true, true), false);
        assert!(state.move_tab("a", "b", "four"));
        assert_eq!(state.group("a").unwrap().active_id(), Some("three"));
        assert_eq!(state.group("b").unwrap().active_id(), Some("four"));
        assert_eq!(state.active_group(), "b");
    }
}
