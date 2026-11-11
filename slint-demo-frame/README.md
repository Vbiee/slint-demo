# Slint Basic Demo

A small native Windows desktop app that demonstrates Slint's core workflow:

- The visual layout is declarative in `ui/app-window.slint`.
- `src/core.rs` holds application state and rules with no UI dependency.
- `src/main.rs` is a thin adapter: it converts core state to Slint properties
  and maps UI callbacks back to core commands.

The task list demonstrates Slint's reactive properties, callbacks, standard
widgets, dynamic model data, and automatic refresh after a Rust state change.

## Requirements

- Rust 1.92+ (`rustc --version`)
- Windows C++ Build Tools / SDK already required by the Rust MSVC toolchain

Slint is a Cargo dependency; no global Slint installation is required.

For editor completion, diagnostics, and live preview, install the Slint editor
extension or `slint-lsp`. See the [official setup guide](https://docs.slint.dev/latest/docs/slint/guide/getting-started/).

## Run

```powershell
cargo run
cargo build --release
```

The release executable is `target\release\slint-basic-demo.exe`. It uses the
Windows GUI subsystem, so it does not open a console window when launched from
Explorer.

The repository enables static MSVC CRT linking (`/MT` equivalent) in
`.cargo/config.toml`; the release executable does not require the Visual C++
Redistributable. Windows system DLLs such as `user32.dll`, `dwmapi.dll`, and
graphics drivers are still supplied by Windows itself.

## Windows title bar and shadow

`src/ui_slint_win32_frame.rs` is a standalone Windows-only bridge. It extends the
app client area over the native caption and implements hit testing for dragging
and resizing. The Slint header can therefore replace the title bar.

The bridge uses Slint's `unstable-winit-030` feature and Win32 APIs, so it is
intentionally isolated in the Windows-only module. Other platforms keep their
normal native window chrome.

### Reuse in another Slint app

Copy `src/ui_slint_win32_frame.rs`, enable Slint's `unstable-winit-030` Cargo feature,
and call these three APIs:

```rust
ui_slint_win32_frame::select_alpha_winit_backend()?; // before AppWindow::new()
let frame = ui_slint_win32_frame::FrameConfig {
    title_bar_height: 64,
    caption_button_width: 156,
    border_color: Some(0x0021_1a_17), // COLORREF: RGB(23, 26, 33)
    ..Default::default()
};
ui_slint_win32_frame::install(&ui.window(), frame);  // before ui.show()
ui_slint_win32_frame::set_corner_radius(&ui.window(), radius_px);
```

`radius_px == 0.0` keeps the 1px DWM seam for native shadow; a positive value
uses a clean alpha corner. `FrameConfig` supplies each resize edge, the drag
area, the caption-button exclusion area, and optional native DWM border color.

## What to change first

1. Modify `ui/app-window.slint` to change visual structure and styling.
2. Modify `src/core.rs` to add business rules without touching UI code.
3. Bind any new core command or view state in `src/main.rs`.
