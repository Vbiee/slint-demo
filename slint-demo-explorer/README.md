# Slint Explorer Demo

A native Slint file-explorer demo that keeps the Explorer UI reusable instead
of putting it in the application caller.

On Windows it also uses the same `ui_slint_win32_frame` module as
`slint-basic-demo`: the Slint titlebar drives Min/Max/Close while Win32 handles
native resizing, dragging, DWM border color, and the corner/shadow behavior.

## Structure

- `src/explorer_model.rs`: filesystem navigation and entries; no Slint/UI code.
- `ui/ui_tree.slint`: data-driven navigation tree. It accepts `TreeNode` data
  and emits selection/expansion callbacks only.
- `ui/ui_list.slint`: data-driven details list. It accepts `ListEntry` data
  and emits selection/activation callbacks only.
- `src/main.rs`: thin host adapter. It maps the filesystem model to component
  data and consumes the emitted IDs.

To reuse `UiTree` or `UiList`, import its `.slint` file in another window,
provide the component's data properties, and handle its callbacks. Neither
component reads the filesystem or performs application commands.

## Build

```powershell
cargo run
cargo build --release
```

If compiling from the VirtualBox shared folder causes `LNK1136`, use a local
target directory:

```powershell
$env:CARGO_TARGET_DIR = "C:\Temp\slint-explorer-demo-target"
cargo run
```
