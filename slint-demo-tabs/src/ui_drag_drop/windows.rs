//! OLE filesystem drop adapter, one instance per HWND, on its owning UI thread.
//! The callback returns true to accept an event. Drop returns COPY only when
//! the callback has actually accepted the payload; it never reports MOVE.

use super::{Event, Point};
use std::{cell::RefCell, path::PathBuf, rc::Rc};
use crate::ui_shell_data::{read_paths, shell_data_object};
use windows::{
    core::{implement, Ref, Result},
    Win32::{
        Foundation::{HWND, POINT, POINTL},
        Graphics::Gdi::ScreenToClient,
        System::{
            Com::IDataObject,
            Ole::{DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget,
                IDropSource, IDropTarget_Impl, OleInitialize, OleUninitialize, RegisterDragDrop,
                RevokeDragDrop},
            SystemServices::MODIFIERKEYS_FLAGS,
        },
        UI::Shell::SHDoDragDrop,
    },
};

type Handler = dyn Fn(Event) -> bool;

#[implement(IDropTarget)]
struct DropTarget {
    hwnd: HWND,
    paths: RefCell<Vec<PathBuf>>,
    handler: Rc<Handler>,
}

fn client_point(hwnd: HWND, point: &POINTL) -> Point {
    let mut position = POINT { x: point.x, y: point.y };
    unsafe { let _ = ScreenToClient(hwnd, &mut position); }
    Point { x: position.x, y: position.y }
}


fn notify(handler: &Handler, event: Event) -> bool {
    // Never unwind Rust panics through an OLE/COM callback.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(event))).unwrap_or(false)
}

unsafe fn accepts_copy(effect: *mut DROPEFFECT) -> bool {
    !effect.is_null() && unsafe { (*effect).0 & DROPEFFECT_COPY.0 != 0 }
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(&self, data: Ref<IDataObject>, _: MODIFIERKEYS_FLAGS, point: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        let paths = data.ok().map(read_paths).unwrap_or_default();
        *self.paths.borrow_mut() = paths.clone();
        let accepted = unsafe { accepts_copy(effect) } && !paths.is_empty() && notify(&*self.handler, Event::Enter { paths, position: client_point(self.hwnd, point) });
        unsafe { *effect = if accepted { DROPEFFECT_COPY } else { DROPEFFECT_NONE }; }
        Ok(())
    }
    fn DragOver(&self, _: MODIFIERKEYS_FLAGS, point: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        let has_paths = !self.paths.borrow().is_empty();
        let accepted = unsafe { accepts_copy(effect) } && has_paths && notify(&*self.handler, Event::Over { position: client_point(self.hwnd, point) });
        unsafe { *effect = if accepted { DROPEFFECT_COPY } else { DROPEFFECT_NONE }; }
        Ok(())
    }
    fn DragLeave(&self) -> Result<()> {
        self.paths.borrow_mut().clear();
        notify(&*self.handler, Event::Leave);
        Ok(())
    }
    fn Drop(&self, data: Ref<IDataObject>, _: MODIFIERKEYS_FLAGS, point: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        let paths = data.ok().map(read_paths).unwrap_or_default();
        self.paths.borrow_mut().clear();
        let accepted = unsafe { accepts_copy(effect) } && !paths.is_empty() && notify(&*self.handler, Event::Drop { paths, position: client_point(self.hwnd, point) });
        unsafe { *effect = if accepted { DROPEFFECT_COPY } else { DROPEFFECT_NONE }; }
        Ok(())
    }
}

/// RAII registration; create/drop on the owning window thread. Do not clone.
pub struct WindowDropTarget { hwnd: HWND, _target: IDropTarget }

impl WindowDropTarget {
    /// `handler` must be fast and reentrancy-safe: OLE calls it synchronously.
    pub fn register(hwnd: HWND, handler: impl Fn(Event) -> bool + 'static) -> Result<Self> {
        unsafe { OleInitialize(None)?; }
        let target: IDropTarget = DropTarget { hwnd, paths: RefCell::new(Vec::new()), handler: Rc::new(handler) }.into();
        if let Err(error) = unsafe { RegisterDragDrop(hwnd, &target) } {
            unsafe { OleUninitialize(); }
            return Err(error);
        }
        Ok(Self { hwnd, _target: target })
    }
}

impl Drop for WindowDropTarget {
    fn drop(&mut self) {
        unsafe { let _ = RevokeDragDrop(self.hwnd); OleUninitialize(); }
    }
}

/// Ask the Windows Shell to start a native copy drag for existing paths sharing
/// a parent directory. Must run on the OLE UI thread while the mouse is down.
/// The Shell owns its drag loop; caller must not delete/move sources on return.

pub fn start_file_drag(hwnd: HWND, paths: &[PathBuf]) -> std::result::Result<bool, String> {
    let data = shell_data_object(paths)?;
    let effect = unsafe { SHDoDragDrop(Some(hwnd), &data, None::<&IDropSource>, DROPEFFECT_COPY) }
        .map_err(|error| error.to_string())?;
    Ok(effect == DROPEFFECT_COPY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_data_round_trips_real_path_and_rejects_invalid_input() {
        unsafe { OleInitialize(None).unwrap(); }
        let path = std::env::current_exe().unwrap();
        let data = shell_data_object(std::slice::from_ref(&path)).unwrap();
        let read = read_paths(&data);
        assert_eq!(read.len(), 1);
        assert!(read[0].to_string_lossy().eq_ignore_ascii_case(&path.to_string_lossy()));
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let target: IDropTarget = DropTarget {
            hwnd: HWND::default(), paths: RefCell::new(Vec::new()),
            handler: Rc::new(move |event| { observed.borrow_mut().push(event); true }),
        }.into();
        let point = POINTL { x: 4, y: 8 };
        let mut effect = DROPEFFECT_COPY;
        unsafe {
            target.DragEnter(&data, MODIFIERKEYS_FLAGS(0), point, &mut effect).unwrap();
            assert_eq!(effect, DROPEFFECT_COPY);
            target.DragOver(MODIFIERKEYS_FLAGS(0), point, &mut effect).unwrap();
            target.Drop(&data, MODIFIERKEYS_FLAGS(0), point, &mut effect).unwrap();
        }
        assert!(matches!(events.borrow().last(), Some(Event::Drop { paths, .. })
            if paths.len() == 1 && paths[0].to_string_lossy().eq_ignore_ascii_case(&path.to_string_lossy())));
        let directory = path.parent().unwrap().to_path_buf();
        let directory_data = shell_data_object(std::slice::from_ref(&directory)).unwrap();
        let directories = read_paths(&directory_data);
        assert_eq!(directories.len(), 1);
        assert!(directories[0].to_string_lossy().eq_ignore_ascii_case(&directory.to_string_lossy()));
        assert!(shell_data_object(&[]).is_err());
        assert!(shell_data_object(&[PathBuf::from("relative-file")]).is_err());
        drop(directory_data);
        drop(target);
        drop(data);
        unsafe { OleUninitialize(); }
    }

    #[test]
    fn registration_is_exclusive_and_released_by_guard() {
        use windows::{core::w, Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPED,
        }};
        let hwnd = unsafe { CreateWindowExW(WINDOW_EX_STYLE(0), w!("STATIC"), w!(""),
            WS_OVERLAPPED, 0, 0, 100, 100, None, None, None, None).unwrap() };
        let first = WindowDropTarget::register(hwnd, |_| true).unwrap();
        assert!(WindowDropTarget::register(hwnd, |_| true).is_err());
        drop(first);
        let second = WindowDropTarget::register(hwnd, |_| true).unwrap();
        drop(second);
        unsafe { DestroyWindow(hwnd).unwrap(); }
    }
}
