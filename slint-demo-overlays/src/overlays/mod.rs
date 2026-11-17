//! Reusable overlay/window adapters.
//!
//! Each module owns its presentation-specific state and translates primitive
//! Slint events into semantic events for the application caller.

pub mod context_menu;
pub mod modal;
pub mod popup;
