//! Shared Windows Shell filesystem-data conversion for clipboard and drag/drop.
//! Requires the windows crate's Foundation, Com, Ole, Shell and Shell_Common
//! features. The caller must use an OLE-initialized thread. No UI dependency.
#![cfg(target_os = "windows")]

use std::{ffi::OsString, os::windows::ffi::{OsStrExt, OsStringExt}, path::{Path, PathBuf}};
use windows::{core::PCWSTR, Win32::{Foundation::S_OK, System::{Com::{CoTaskMemFree, DVASPECT_CONTENT, FORMATETC, IBindCtx, IDataObject, TYMED_HGLOBAL}, Ole::{CF_HDROP, ReleaseStgMedium}}, UI::Shell::{Common::ITEMIDLIST, DragQueryFileW, HDROP, ILFindLastID, SHCreateDataObject, SHParseDisplayName}}};

/// Read real filesystem paths from CF_HDROP. Virtual Shell objects are not supported.
pub fn read_paths(data: &IDataObject) -> Vec<PathBuf> {
    let format = FORMATETC { cfFormat: CF_HDROP.0, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex: -1, tymed: TYMED_HGLOBAL.0 as u32 };
    unsafe {
        if data.QueryGetData(&format) != S_OK { return Vec::new(); }
        let Ok(mut medium) = data.GetData(&format) else { return Vec::new(); };
        if medium.tymed != TYMED_HGLOBAL.0 as u32 || medium.u.hGlobal.is_invalid() { ReleaseStgMedium(&mut medium); return Vec::new(); }
        let hdrop = HDROP(medium.u.hGlobal.0);
        let count = DragQueryFileW(hdrop, u32::MAX, None);
        let mut paths = Vec::with_capacity(count as usize);
        for index in 0..count {
            let length = DragQueryFileW(hdrop, index, None) as usize;
            let mut buffer = vec![0u16; length + 1];
            if DragQueryFileW(hdrop, index, Some(&mut buffer)) > 0 { paths.push(PathBuf::from(OsString::from_wide(&buffer[..length]))); }
        }
        ReleaseStgMedium(&mut medium);
        paths
    }
}

/// Create Shell data for existing absolute paths with one shared parent.
pub fn shell_data_object(paths: &[PathBuf]) -> std::result::Result<IDataObject, String> {
    let Some(parent) = paths.first().and_then(|path| path.parent()) else { return Err("No file paths".into()); };
    if paths.iter().any(|path| !path.is_absolute() || path.parent() != Some(parent) || !path.exists()) {
        return Err("File paths must exist and share one parent folder".into());
    }
    struct Pidl(*mut ITEMIDLIST);
    impl Drop for Pidl { fn drop(&mut self) { unsafe { CoTaskMemFree(Some(self.0.cast())); } } }
    fn parse(path: &Path) -> std::result::Result<Pidl, String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let mut id = std::ptr::null_mut();
        unsafe { SHParseDisplayName(PCWSTR(wide.as_ptr()), None::<&IBindCtx>, &mut id, 0, None) }.map_err(|error| error.to_string())?;
        if id.is_null() { return Err("Shell returned an empty item ID".into()); }
        Ok(Pidl(id))
    }
    let parent_id = parse(parent)?;
    let item_ids = paths.iter().map(|path| parse(path)).collect::<std::result::Result<Vec<_>, _>>()?;
    let children: Vec<*const ITEMIDLIST> = item_ids.iter().map(|id| unsafe { ILFindLastID(id.0) as *const _ }).collect();
    unsafe { SHCreateDataObject(Some(parent_id.0), Some(&children), None::<&IDataObject>) }.map_err(|error| error.to_string())
}
