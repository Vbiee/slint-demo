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
- `ui/ui_list.slint`: generic, virtualized table. The caller supplies a
  `ListColumn` schema and `ListRow` cell data; it emits selection, activation,
  and column-resize callbacks only.
- `src/main.rs`: thin Explorer host adapter. It maps filesystem data to the
  generated `AppWindow` properties and consumes the emitted IDs. This is the
  only place that knows the concrete `AppWindow` type.

To reuse `UiTree` or `UiList`, import its `.slint` file in another window,
provide the component's data properties, and handle its callbacks. If Rust
helpers are wanted, implement the small `ui_tree::Port` and/or `ui_list::Port`
for that window next to its generated type. Neither component reads the
filesystem or performs application commands.

`UiList` uses the column schema as its rendering contract. Each column has an
ID, title, fixed or auto-stretch width (`-1px`), minimum width, alignment, and
resizability. It may also provide default text/background colors for its header
and cells. A cell may override either color for that particular row/column and
may carry a caller-owned `slint::Image` plus its requested size; `UiList` only
lays it out before the cell text. Missing cells render empty.

`UiList` also has an optional status slot. The caller supplies its complete
Slint content (text, images, buttons, layout, and styling), while `UiList`
only reserves its height and places it above or below the table. The component
does not derive a row count or any application status by itself.

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
