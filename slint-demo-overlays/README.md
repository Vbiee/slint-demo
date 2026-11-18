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

The demo buttons open each implemented kind directly: an in-app modal, a
detached native context menu, and a detached native Slint window. Right-clicking
the canvas opens the native context menu at the pointer position; activating
another window dismisses it.

`ContextMenuWindowManager::bind_to_caller()` owns the caller association. When
the menu loses focus while the right mouse button is down over that caller, it
replaces the menu at the pointer immediately without a caller-side workaround.

`ContextMenuWindowManager::new()` accepts `ContextMenuOptions { style }`.
`ContextMenuStyle::default()` uses the reusable Win32 frame and native DWM
shadow. A positive `style.corner_radius` uses the frameless rounded
presentation instead. The style also exposes menu/entry dimensions, padding,
backgrounds, borders, text colors, hover color, and `entry_corner_radius`.

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
