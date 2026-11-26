//! Window-level drag/drop contract. It is independent of list/tree and of the
//! application model: hosts decide which target accepts a point and what a
//! dropped payload means. The Windows adapter currently supports filesystem
//! paths; virtual Shell objects require a separate payload implementation.
//!
//! Windows integration requires an OLE-initialized UI thread, a live HWND and
//! a message loop. Only one target may be registered per HWND. The host must
//! disable winit's built-in drop target before creating that window and retain
//! the returned guard on the same thread until before the HWND is destroyed.
//! With Slint/winit use WindowAttributesExtWindows::with_drag_and_drop(false)
//! in the same backend attributes hook as other window settings. Window creation
//! can be deferred until the event loop; install only after an HWND exists.
//! Do not overwrite the frame's single `on_winit_window_event` filter.
//!
//! `Event` contains filesystem paths and physical client coordinates. The host
//! converts coordinates to UI logical units, hit-tests its own controls and
//! returns true only for an accepted target/drop. Windows reports COPY only;
//! no filesystem operation is performed by this module. `start_file_drag`
//! supplies actual Shell objects for existing paths with a shared parent, starts
//! a copy drag while the button is down and returns completed/cancelled/error.
//! MOVE/LINK, virtual files, arbitrary payload formats and other platforms are
//! not implemented. A caller must not treat COPY as a request to delete source.

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Physical client coordinates, before conversion by the host UI scale.
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Cursor entered the window with a filesystem payload.
    Enter { paths: Vec<PathBuf>, position: Point },
    /// Cursor moved while the filesystem payload is over the window.
    Over { position: Point },
    /// Drag left without dropping; clear any visual feedback.
    Leave,
    /// Filesystem payload dropped; callback decides if its target accepts it.
    Drop { paths: Vec<PathBuf>, position: Point },
}

// Windows-specific adapter is separate so other UI controls can share it.
#[cfg(target_os = "windows")]
pub mod windows;
