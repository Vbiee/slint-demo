//! Caller-facing adapter for a detached native context-menu window.

use crate::{AppWindow, ContextMenuItem, DetachedContextMenu};
use slint::{
    winit_030::{winit::event::WindowEvent, EventResult, WinitWindowAccessor},
    ComponentHandle, ModelRc, PhysicalPosition, SharedString, VecModel,
};
use std::{cell::Cell, rc::Rc};

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

/// Presentation for the detached context menu.
///
/// A zero radius uses a custom Win32 frame and its DWM shadow. Any positive
/// radius uses a frameless alpha-edge window, so Windows does not add a square
/// native shadow around the rounded menu.
#[derive(Clone, Copy, Debug)]
pub struct ContextMenuOptions {
    pub corner_radius: f32,
}

impl Default for ContextMenuOptions {
    fn default() -> Self {
        Self { corner_radius: 0.0 }
    }
}

pub struct ContextMenuWindowManager {
    menu: DetachedContextMenu,
    visible: Rc<Cell<bool>>,
    native_frame: Option<crate::ui_slint_win32_frame::FrameConfig>,
    corner_radius: f32,
}

impl ContextMenuWindowManager {
    pub fn new(
        options: ContextMenuOptions,
        on_event: impl Fn(Event) + 'static,
    ) -> Result<Self, slint::PlatformError> {
        let menu = DetachedContextMenu::new()?;
        let corner_radius = options.corner_radius.max(0.0);
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
        menu.set_use_native_frame(native_frame.is_some());
        menu.set_corner_radius(corner_radius);
        if let Some(config) = native_frame {
            crate::ui_slint_win32_frame::install(&menu.window(), config);
        }
        let visible = Rc::new(Cell::new(false));
        let on_event = Rc::new(on_event);

        menu.on_item_triggered({
            let menu_weak = menu.as_weak();
            let visible = visible.clone();
            let on_event = on_event.clone();
            move |id: SharedString| {
                if visible.replace(false) {
                    if let Some(menu) = menu_weak.upgrade() {
                        menu.hide().expect("failed to hide detached context menu");
                    }
                    on_event(Event::ItemTriggered(id.to_string()));
                }
            }
        });

        // A native context menu is dismissed when the user activates another
        // window, including its caller.
        menu.window().on_winit_window_event({
            let menu_weak = menu.as_weak();
            let visible = visible.clone();
            move |_window, event| {
                if matches!(event, WindowEvent::Focused(false)) && visible.replace(false) {
                    if let Some(menu) = menu_weak.upgrade() {
                        menu.hide().expect("failed to hide detached context menu");
                    }
                    on_event(Event::Dismissed);
                }
                EventResult::Propagate
            }
        });

        Ok(Self {
            menu,
            visible,
            native_frame,
            corner_radius,
        })
    }

    pub fn show(&self, caller: &AppWindow, x: f32, y: f32, items: Vec<Item>) -> Result<(), slint::PlatformError> {
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

        let caller_position = caller.window().position();
        let scale = caller.window().scale_factor();
        self.menu.window().set_position(PhysicalPosition::new(
            caller_position.x + (x * scale).round() as i32,
            caller_position.y + (y * scale).round() as i32,
        ));
        self.menu.show()?;
        if let Some(config) = self.native_frame {
            crate::ui_slint_win32_frame::install_now(&self.menu.window(), config);
        }
        crate::ui_slint_win32_frame::set_corner_radius(&self.menu.window(), self.corner_radius);
        self.visible.set(true);
        Ok(())
    }
}

