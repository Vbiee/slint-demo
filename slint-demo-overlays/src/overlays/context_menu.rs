//! Caller-facing adapter for a detached native context-menu window.

use crate::{ContextMenuItem, DetachedContextMenu};
use slint::{
    winit_030::{
        winit::event::WindowEvent,
        EventResult, WinitWindowAccessor,
    },
    Color, ComponentHandle, LogicalSize, ModelRc, PhysicalPosition, SharedString, VecModel,
};
use std::{cell::{Cell, RefCell}, rc::Rc, time::Duration};
use windows_sys::Win32::{
    Foundation::POINT,
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_RBUTTON},
        WindowsAndMessaging::GetCursorPos,
    },
};

#[derive(Clone, Default)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub shortcut: String,
    pub enabled: bool,
    pub checked: bool,
    pub danger: bool,
    pub separator: bool,
    /// If present, takes precedence over the theme's normal, danger, and
    /// disabled text colors for this entire row (label, shortcut, checkmark).
    pub text_color: Option<Color>,
    /// If present, replaces the theme's hover fill for this enabled row.
    pub hover_background_color: Option<Color>,
}

pub enum Event {
    ItemTriggered(String),
    Dismissed,
}

/// The small bridge required by the in-scene presentation. Product windows
/// implement this at their boundary; the menu module never needs to know a
/// product's layout or business commands.
pub trait ContextMenuPort: Clone + 'static {
    fn set_context_menu_items(&self, items: ModelRc<ContextMenuItem>);
    fn set_context_menu_anchor(&self, x: f32, y: f32);
    fn set_context_menu_open(&self, open: bool);
    fn on_context_menu_item_triggered(&self, callback: impl Fn(String) + 'static);
    fn on_context_menu_dismissed(&self, callback: impl Fn() + 'static);
    fn on_context_menu_replacement_requested(&self, callback: impl Fn(f32, f32) + 'static);
    fn context_menu_screen_position(&self, x: f32, y: f32) -> PhysicalPosition;
    fn context_menu_contains_screen_position(&self, position: PhysicalPosition) -> bool;
}

/// One API over the two unavoidable visual roots (`Rectangle` versus native
/// `Window`). A single `use_native` flag at show-time selects the presentation.
pub struct ContextMenuManager<P: ContextMenuPort> {
    port: P,
    native: Rc<ContextMenuWindowManager>,
    in_app_items: Rc<RefCell<Vec<Item>>>,
}

impl<P: ContextMenuPort> ContextMenuManager<P> {
    pub fn new(
        port: P,
        options: ContextMenuOptions,
        on_event: impl Fn(Event) + 'static,
    ) -> Result<Self, slint::PlatformError> {
        let on_event = Rc::new(on_event);
        let native = ContextMenuWindowManager::new(options, {
            let on_event = on_event.clone();
            move |event| on_event(event)
        })?;
        native.bind_to_port(port.clone());
        let in_app_items = Rc::new(RefCell::new(Vec::new()));
        let event_port = port.clone();
        let item_port = port.clone();
        port.on_context_menu_item_triggered({
            let on_event = on_event.clone();
            move |id| {
                item_port.set_context_menu_open(false);
                on_event(Event::ItemTriggered(id));
            }
        });
        port.on_context_menu_dismissed(move || {
            event_port.set_context_menu_open(false);
            on_event(Event::Dismissed);
        });
        let replacement_port = port.clone();
        port.on_context_menu_replacement_requested({
            let in_app_items = in_app_items.clone();
            move |x, y| {
                // The overlay consumed this right-click, so replace the old
                // menu itself instead of relying on the host to see a second
                // pointer event. Hosts can still call `show()` with new data
                // whenever their menu contents are context-sensitive.
                replacement_port.set_context_menu_items(ModelRc::new(VecModel::from(
                    to_slint_items(in_app_items.borrow().clone()),
                )));
                replacement_port.set_context_menu_anchor(x, y);
                replacement_port.set_context_menu_open(true);
            }
        });
        Ok(Self { port, native, in_app_items })
    }

    pub fn show(&self, use_native: bool, x: f32, y: f32, items: Vec<Item>) -> Result<(), slint::PlatformError> {
        if use_native {
            self.native.show_at_screen_position(self.port.context_menu_screen_position(x, y), items)
        } else {
            *self.in_app_items.borrow_mut() = items;
            self.port.set_context_menu_items(ModelRc::new(VecModel::from(
                to_slint_items(self.in_app_items.borrow().clone()),
            )));
            self.port.set_context_menu_anchor(x, y);
            self.port.set_context_menu_open(true);
            Ok(())
        }
    }

}

fn to_slint_items(items: Vec<Item>) -> Vec<ContextMenuItem> {
    items.into_iter().map(|item| ContextMenuItem {
        id: item.id.into(),
        label: item.label.into(),
        shortcut: item.shortcut.into(),
        icon: slint::Image::default(),
        has_icon: false,
        enabled: item.enabled,
        checked: item.checked,
        danger: item.danger,
        separator: item.separator,
        text_color: item.text_color.unwrap_or_else(transparent_color),
        has_text_color: item.text_color.is_some(),
        hover_background_color: item.hover_background_color.unwrap_or_else(transparent_color),
        has_hover_background_color: item.hover_background_color.is_some(),
    }).collect()
}

fn transparent_color() -> Color {
    Color::from_argb_u8(0, 0, 0, 0)
}

/// All visual values for a context menu. Omit this from `ContextMenuOptions`
/// to use the current default theme.
#[derive(Clone, Debug)]
pub struct ContextMenuStyle {
    pub menu_width: f32,
    pub menu_max_height: f32,
    pub menu_padding: f32,
    pub entry_spacing: f32,
    pub menu_border_width: f32,
    pub menu_background: Color,
    pub menu_border_color: Color,
    pub corner_radius: f32,
    pub entry_height: f32,
    pub entry_corner_radius: f32,
    pub entry_text_left_padding: f32,
    pub entry_border_width: f32,
    pub entry_background: Color,
    pub entry_border_color: Color,
    pub entry_hover_background: Color,
    pub entry_text_color: Color,
    pub entry_shortcut_color: Color,
    pub entry_disabled_text_color: Color,
    pub entry_danger_text_color: Color,
}

impl Default for ContextMenuStyle {
    fn default() -> Self {
        Self {
            menu_width: 204.0,
            menu_max_height: 320.0,
            menu_padding: 6.0,
            entry_spacing: 2.0,
            menu_border_width: 1.0,
            menu_background: Color::from_rgb_u8(0x2a, 0x31, 0x3d),
            menu_border_color: Color::from_rgb_u8(0x4a, 0x57, 0x6b),
            corner_radius: 0.0,
            entry_height: 32.0,
            entry_corner_radius: 4.0,
            entry_text_left_padding: 10.0,
            entry_border_width: 0.0,
            entry_background: Color::from_argb_u8(0, 0, 0, 0),
            entry_border_color: Color::from_argb_u8(0, 0, 0, 0),
            entry_hover_background: Color::from_rgb_u8(0x3b, 0x4d, 0x64),
            entry_text_color: Color::from_rgb_u8(0xe7, 0xed, 0xf6),
            entry_shortcut_color: Color::from_rgb_u8(0x9e, 0xab, 0xba),
            entry_disabled_text_color: Color::from_rgb_u8(0x6e, 0x78, 0x88),
            entry_danger_text_color: Color::from_rgb_u8(0xff, 0x9a, 0x9a),
        }
    }
}

/// Presentation for the detached context menu.
#[derive(Clone, Debug, Default)]
pub struct ContextMenuOptions {
    pub style: ContextMenuStyle,
}

type ReopenCallback = Box<dyn FnMut(i32, i32) -> bool>;

pub struct ContextMenuWindowManager {
    menu: DetachedContextMenu,
    visible: Rc<Cell<bool>>,
    ignore_next_focus_loss: Rc<Cell<bool>>,
    latest_items: Rc<RefCell<Vec<Item>>>,
    reopen_after_focus_loss: Rc<RefCell<Option<ReopenCallback>>>,
    native_frame: Option<crate::ui_slint_win32_frame::FrameConfig>,
    corner_radius: f32,
}

fn apply_style(menu: &DetachedContextMenu, style: &ContextMenuStyle, use_native_frame: bool) {
    menu.set_use_native_frame(use_native_frame);
    menu.set_corner_radius(style.corner_radius.max(0.0));
    menu.set_menu_width(style.menu_width.max(1.0));
    menu.set_menu_max_height(style.menu_max_height.max(1.0));
    menu.set_menu_padding(style.menu_padding.max(0.0));
    menu.set_entry_spacing(style.entry_spacing.max(0.0));
    menu.set_menu_border_width(style.menu_border_width.max(0.0));
    menu.set_menu_background(style.menu_background);
    menu.set_menu_border_color(style.menu_border_color);
    menu.set_entry_height(style.entry_height.max(1.0));
    menu.set_entry_corner_radius(style.entry_corner_radius.max(0.0));
    menu.set_entry_text_left_padding(style.entry_text_left_padding.max(0.0));
    menu.set_entry_border_width(style.entry_border_width.max(0.0));
    menu.set_entry_background(style.entry_background);
    menu.set_entry_border_color(style.entry_border_color);
    menu.set_entry_hover_background(style.entry_hover_background);
    menu.set_entry_text_color(style.entry_text_color);
    menu.set_entry_shortcut_color(style.entry_shortcut_color);
    menu.set_entry_disabled_text_color(style.entry_disabled_text_color);
    menu.set_entry_danger_text_color(style.entry_danger_text_color);
}

/// `items-layout.preferred-height` is computed by Slint's layout pass. Resize
/// once before showing and once on the following event-loop turn, so the first
/// opening cannot retain the empty-model measurement.
fn resize_window_to_content(
    menu: &DetachedContextMenu,
    native_frame: Option<crate::ui_slint_win32_frame::FrameConfig>,
    corner_radius: f32,
) {
    let content_width = menu.get_menu_width();
    let content_height = menu.get_bounded_intrinsic_height();
    menu.window().set_size(LogicalSize::new(content_width, content_height));
    if let Some(config) = native_frame {
        crate::ui_slint_win32_frame::install_now(&menu.window(), config);
    }
    crate::ui_slint_win32_frame::set_corner_radius(&menu.window(), corner_radius);
    if native_frame.is_some() {
        let scale = menu.window().scale_factor();
        crate::ui_slint_win32_frame::set_custom_frame_client_size(
            &menu.window(),
            (content_width * scale).round().max(1.0) as u32,
            (content_height * scale).round().max(1.0) as u32,
        );
    }
}

impl ContextMenuWindowManager {
    pub fn new(
        options: ContextMenuOptions,
        on_event: impl Fn(Event) + 'static,
    ) -> Result<Rc<Self>, slint::PlatformError> {
        let menu = DetachedContextMenu::new()?;
        let style = options.style;
        let corner_radius = style.corner_radius.max(0.0);
        // Square menus retain the custom native frame/DWM shadow. A positive
        // radius uses the alpha-capable frameless path from the frame demo;
        // its transparent corner pixels are necessary for a clean round edge.
        let native_frame = (corner_radius == 0.0).then_some(
            crate::ui_slint_win32_frame::FrameConfig {
                resize_left: 0,
                resize_top: 0,
                resize_right: 0,
                resize_bottom: 0,
                title_bar_height: 0,
                caption_button_width: 0,
                border_color: None,
            },
        );
        apply_style(&menu, &style, native_frame.is_some());
        if let Some(config) = native_frame {
            crate::ui_slint_win32_frame::install(&menu.window(), config);
        }

        let visible = Rc::new(Cell::new(false));
        let ignore_next_focus_loss = Rc::new(Cell::new(false));
        let latest_items = Rc::new(RefCell::new(Vec::new()));
        let reopen_after_focus_loss = Rc::new(RefCell::new(None::<ReopenCallback>));
        let on_event = Rc::new(on_event);

        menu.on_item_triggered({
            let menu_weak = menu.as_weak();
            let visible = visible.clone();
            let on_event = on_event.clone();
            move |id: SharedString| {
                if visible.replace(false) {
                    if let Some(menu) = menu_weak.upgrade() {
                        let _ = menu.hide();
                    }
                    on_event(Event::ItemTriggered(id.to_string()));
                }
            }
        });

        menu.window().on_winit_window_event({
            let menu_weak = menu.as_weak();
            let visible = visible.clone();
            let ignore_next_focus_loss = ignore_next_focus_loss.clone();
            let reopen_after_focus_loss = reopen_after_focus_loss.clone();
            move |_window, event| {
                if !matches!(event, WindowEvent::Focused(false)) || !visible.get() {
                    return EventResult::Propagate;
                }
                if ignore_next_focus_loss.replace(false) {
                    return EventResult::Propagate;
                }

                // A right-click in the caller transfers focus away before its
                // host can receive another context-menu request. Turn that
                // transition into an in-place replacement instead of a plain
                // dismiss that eats the first right-click.
                let mut point = POINT::default();
                let right_button_down = unsafe { GetAsyncKeyState(VK_RBUTTON as i32) } < 0;
                let cursor_available = unsafe { GetCursorPos(&mut point) } != 0;
                if right_button_down && cursor_available {
                    visible.set(false);
                    if let Some(menu) = menu_weak.upgrade() { let _ = menu.hide(); }
                    if let Some(reopen) = reopen_after_focus_loss.borrow_mut().as_mut()
                        && reopen(point.x, point.y)
                    {
                        return EventResult::Propagate;
                    }
                    visible.set(true);
                }

                if visible.replace(false) {
                    if let Some(menu) = menu_weak.upgrade() {
                        let _ = menu.hide();
                    }
                    on_event(Event::Dismissed);
                }
                EventResult::Propagate
            }
        });

        Ok(Rc::new(Self {
            menu,
            visible,
            ignore_next_focus_loss,
            latest_items,
            reopen_after_focus_loss,
            native_frame,
            corner_radius,
        }))
    }

    fn bind_to_port<P: ContextMenuPort>(self: &Rc<Self>, port: P) {
        let manager = Rc::downgrade(self);
        *self.reopen_after_focus_loss.borrow_mut() = Some(Box::new(move |x, y| {
            let position = PhysicalPosition::new(x, y);
            if !port.context_menu_contains_screen_position(position) { return false; }
            manager.upgrade()
                .is_some_and(|manager| manager.show_last_at_screen_position(position).is_ok())
        }));
    }

    pub fn show_at_screen_position(
        &self,
        position: PhysicalPosition,
        items: Vec<Item>,
    ) -> Result<(), slint::PlatformError> {
        *self.latest_items.borrow_mut() = items;
        self.show_last_at_screen_position(position)
    }

    fn show_last_at_screen_position(&self, position: PhysicalPosition) -> Result<(), slint::PlatformError> {
        if self.visible.replace(false) {
            self.ignore_next_focus_loss.set(true);
            let _ = self.menu.hide();
        }
        // Native and in-app menus receive precisely the same item contract.
        let items = to_slint_items(self.latest_items.borrow().clone());
        self.menu.set_items(ModelRc::new(VecModel::from(items)));
        // Both presentations query the same shared surface's intrinsic size.
        // Resize after assigning the model because a Window was constructed
        // before its item model existed.
        resize_window_to_content(&self.menu, self.native_frame, self.corner_radius);
        self.menu.window().set_position(position);
        self.menu.show()?;
        resize_window_to_content(&self.menu, self.native_frame, self.corner_radius);
        let menu = self.menu.as_weak();
        let native_frame = self.native_frame;
        let corner_radius = self.corner_radius;
        slint::Timer::single_shot(Duration::from_millis(0), move || {
            if let Some(menu) = menu.upgrade() {
                resize_window_to_content(&menu, native_frame, corner_radius);
            }
        });
        self.visible.set(true);
        Ok(())
    }
}
