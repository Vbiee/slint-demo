//! Reusable Windows clipboard adapter for copying filesystem items.
//!
//! Requires sibling `ui_shell_data.rs`, Windows `windows` crate features
//! Foundation, Com, Ole, Shell and Shell_Common, and an OLE-compatible UI
//! thread. No HWND, Slint, list/tree, or filesystem mutation is required.
//! `set_files` replaces the system clipboard and flushes it so Explorer can
//! paste after this process exits. Inputs must exist, be absolute and share a
//! parent. `get_files` reads CF_HDROP paths; empty means unsupported/no files.
//! Only copy is supported: do not use for cut/move or delete the source.
//! Virtual files, text, images and other platforms are not yet supported.

#[cfg(target_os = "windows")]
pub mod windows {
    use std::path::PathBuf;
    use windows::Win32::System::Ole::{OleFlushClipboard, OleGetClipboard, OleInitialize, OleSetClipboard, OleUninitialize};
    use crate::ui_shell_data::{read_paths, shell_data_object};

    struct OleGuard;
    impl OleGuard {
        fn new() -> Result<Self, String> { unsafe { OleInitialize(None) }.map_err(|e| e.to_string())?; Ok(Self) }
    }
    impl Drop for OleGuard { fn drop(&mut self) { unsafe { OleUninitialize() } } }

    /// Replace the clipboard with file Shell data. May fail if it is busy.
    pub fn set_files(paths: &[PathBuf]) -> Result<(), String> {
        let _ole = OleGuard::new()?;
        let data = shell_data_object(paths)?;
        unsafe { OleSetClipboard(&data) }.map_err(|e| e.to_string())?;
        // A failed flush still means the clipboard was changed; report that
        // explicitly because data may no longer survive process shutdown.
        unsafe { OleFlushClipboard() }.map_err(|e| format!("Clipboard set, but flush failed: {e}"))
    }

    /// Read filesystem paths without modifying the clipboard or the files.
    pub fn get_files() -> Result<Vec<PathBuf>, String> {
        let _ole = OleGuard::new()?;
        let data = unsafe { OleGetClipboard() }.map_err(|e| e.to_string())?;
        Ok(read_paths(&data))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // These invalid inputs are rejected before OleSetClipboard, so the
        // tests never overwrite the user's actual Windows clipboard.
        #[test]
        fn invalid_inputs_are_rejected_without_changing_clipboard() {
            assert!(set_files(&[]).is_err());
            assert!(set_files(&[PathBuf::from("relative-file")]).is_err());
        }
    }
}
