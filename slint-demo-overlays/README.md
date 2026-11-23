# Slint overlay demo

Demo riêng cho modal popup và context menu, đặt trong `slint-basic-demo`.

## Run

```powershell
cd D:\source\work\.repos\slint-basic-demo\slint-demo-overlays
cargo run
```

Right-click vùng canvas để mở context menu, hoặc bấm **Open modal**.

## Structure

## Detached native popup

`ui/detached_popup.slint` exports a separate Slint `Window`, not an overlay
inside `AppWindow`. `src/popup_window_manager.rs` owns it and exposes
`show(title, message)`, while emitting `Accepted` or `Dismissed` to the caller.

It is the same architecture class as the ImGui packager's `FloatingWindows`:
the main UI remains interactive. OS-level parent ownership and blocking modal
behavior are platform-specific additions, not part of this reusable baseline.

When **Lock caller** is enabled, the caller receives a transparent input
scrim while the detached popup is open. Clicking that blocked UI restores the
popup if needed, focuses it, and flashes its Windows caption three times.

The demo buttons open each detached presentation directly: a native context
menu and a detached native Slint window. Right-clicking the canvas opens the
reusable in-app menu at the pointer position; activating another window still
dismisses the detached native menu.

`ContextMenuManager` owns the caller association through a small port. The
caller creates one manager and selects the presentation with the one
`use_native` flag passed to `show(use_native, x, y, items)`; both paths emit
the same semantic events.

The reusable Rust core (`src/overlays/context_menu.rs`) has no `AppWindow`
dependency. Each host implements `ContextMenuPort` in its own thin adapter;
this demo's adapter is `src/app_window_context_menu_port.rs`. Moving the
module to another application therefore requires adding that host adapter,
not changing the menu module or either menu presentation.

`ContextMenuWindowManager::new()` accepts `ContextMenuOptions { style }`.
`ContextMenuStyle::default()` uses the reusable Win32 frame and native DWM
shadow. A positive `style.corner_radius` uses the frameless rounded
presentation instead. The style also exposes menu/entry dimensions, padding,
backgrounds, borders, text colors, hover color, and `entry_corner_radius`.

## Reusable in-app context menu

`ui/ui_context_menu.slint` is independent of `AppWindow` and of the Windows
native manager. Embed `UiContextMenu` as a full-size overlay, pass `open`,
`anchor-x`, `anchor-y`, and a caller-owned `[ContextMenuItem]` model, then
handle only `item-triggered(id)` and `dismissed()`. Its item contract supports
enabled/disabled and danger states, optional icons, check marks, shortcuts and
separators. Geometry and colors are properties rather than application
constants; long menus keep a bounded surface and scroll their item viewport.

The demo opens this reusable in-app menu on right-click inside the canvas. The
button still opens the detached native presentation, which is deliberately a
separate Windows-specific adapter.

```text
ui/ui_modal.slint         visual + primitive input của modal
ui/ui_context_menu.slint  visual + primitive input của menu
src/overlays/modal.rs     modal data/event adapter cho caller Rust
src/overlays/context_menu.rs menu data/event adapter cho caller Rust
src/overlays/popup.rs     detached popup window manager
src/main.rs               caller demo; chỉ quyết định nghiệp vụ
```

Hai adapter Rust phát semantic event (`Confirmed`, `Cancelled`,
`ItemTriggered`, `Dismissed`). Component Slint không biết thao tác đó có nghĩa
gì trong sản phẩm thật, nên có thể tái sử dụng mà không sửa UI.
