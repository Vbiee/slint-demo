//! Caller-facing adapter for a detached native context-menu window.

use crate::{AppWindow, ContextMenuItem, DetachedContextMenu};
use slint::{
    winit_030::{
        winit::event::WindowEvent,
        EventResult, WinitWindowAccessor,
    },
    Color, ComponentHandle, ModelRc, PhysicalPosition, SharedString, VecModel,
};
use std::{cell::{Cell, RefCell}, rc::Rc};
use windows_sys::Win32::{
    Foundation::POINT,
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_RBUTTON},
        WindowsAndMessaging::GetCursorPos,
    },
};

#[derive(Clone)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub danger: bool,
}

pub enum Event {
    ItemTriggered(String),
    Dismissed,
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
    menu.set_entry_disabled_text_color(style.entry_disabled_text_color);
    menu.set_entry_danger_text_color(style.entry_danger_text_color);
}

impl ContextMenuWindowManager {
    pub fn new(
        options: ContextMenuOptions,
        on_event: impl Fn(Event) + 'static,
    ) -> Result<Self, slint::PlatformError> {
        let menu = DetachedContextMenu::new()?;
        let style = options.style;
        let corner_radius = style.corner_radius.max(0.0);
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

                // Windows delivers focus loss while the button is still down.
                // If the pointer is over the bound caller, replace this menu
                // immediately instead of treating the click as a plain dismiss.
                let mut point = POINT::default();
                let right_button_down = unsafe { GetAsyncKeyState(VK_RBUTTON as i32) } < 0;
                let cursor_available = unsafe { GetCursorPos(&mut point) } != 0;
                if right_button_down && cursor_available {
                    visible.set(false);
                    // Re-create the native presentation so show() activates
                    // the replacement menu instead of leaving the old,
                    // focusless window on screen.
                    if let Some(menu) = menu_weak.upgrade() {
                        let _ = menu.hide();
                    }
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

        Ok(Self {
            menu,
            visible,
            ignore_next_focus_loss,
            reopen_after_focus_loss,
            native_frame,
            corner_radius,
        })
    }

    pub fn show(
        &self,
        caller: &AppWindow,
        x: f32,
        y: f32,
        items: Vec<Item>,
    ) -> Result<(), slint::PlatformError> {
        let caller_position = caller.window().position();
        let scale = caller.window().scale_factor();
        self.show_at_screen_position(
            PhysicalPosition::new(
                caller_position.x + (x * scale).round() as i32,
                caller_position.y + (y * scale).round() as i32,
            ),
            items,
        )
    }

    /// Bind one caller. Right-clicking inside it while this menu loses focus
    /// replaces the menu at the pointer, all within this module.
    pub fn bind_to_caller(
        self: &Rc<Self>,
        caller: &AppWindow,
        items: impl Fn() -> Vec<Item> + 'static,
    ) {
        let manager = Rc::downgrade(self);
        let caller = caller.as_weak();
        let items = Rc::new(items);
        *self.reopen_after_focus_loss.borrow_mut() = Some(Box::new(move |x, y| {
            let Some(manager) = manager.upgrade() else { return false; };
            let Some(caller) = caller.upgrade() else { return false; };
            let position = caller.window().position();
            let size = caller.window().size();
            if x < position.x || y < position.y || x >= position.x + size.width as i32 || y >= position.y + size.height as i32 {
                return false;
            }
            manager
                .show_at_screen_position(PhysicalPosition::new(x, y), items())
                .is_ok()
        }));
    }

    fn show_at_screen_position(
        &self,
        position: PhysicalPosition,
        items: Vec<Item>,
    ) -> Result<(), slint::PlatformError> {
        if self.visible.replace(false) {
            self.ignore_next_focus_loss.set(true);
            let _ = self.menu.hide();
        }
        let items = items
            .into_iter()
            .map(|item| ContextMenuItem {
                id: item.id.into(),
                label: item.label.into(),
                enabled: item.enabled,
                danger: item.danger,
            })
            .collect::<Vec<_>>();
        self.menu.set_items(ModelRc::new(VecModel::from(items)));
        self.menu.window().set_position(position);
        self.menu.show()?;
        if let Some(config) = self.native_frame {
            crate::ui_slint_win32_frame::install_now(&self.menu.window(), config);
        }
        crate::ui_slint_win32_frame::set_corner_radius(&self.menu.window(), self.corner_radius);
        self.visible.set(true);
        Ok(())
    }
}
