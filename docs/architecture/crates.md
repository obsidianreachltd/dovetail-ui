# Crates

## Candidate package map

This is a list of possible packages, not a commitment. The first release uses the smallest subset that works.

| Package | Responsibility |
|---|---|
| `dovetail-ui` (library `dovetail`) | Small facade and common re-exports ([D0006](../decisions/D0006-facade-crate-name.md)) |
| `dovetail-core` | Component contracts, identity, events, reactive state |
| `dovetail-macros` | Declarative syntax and component expansion |
| `dovetail-layout` | Sizing, alignment, layout primitives, scrolling and scrollbars |
| `dovetail-session` | One window's component tree, layout, focus and input, with no window ([D0027](../decisions/D0027-testing-foundations.md)) |
| `dovetail-testing` | Tests for applications: find elements, send input through the window, and move time, with no window or GPU ([D0027](../decisions/D0027-testing-foundations.md)) |
| `dovetail-render` | Rendering interfaces and GPU integration |
| `dovetail-desktop` | Windows, event loop, input, platform integration |
| `dovetail-controls` | Buttons, toggles, check boxes, radio buttons, drop-downs, number fields, sliders, tooltips, tabs, indicators |
| `dovetail-text` | Text, selection, editing |
| `dovetail-data` | Lists, trees and tables that make only the rows in sight, selection, and a property grid |
| `dovetail-image` | PNG, JPEG and GIF decoding, GIF and APNG animation, icons from SVG outlines, and Dovetail's icon set |
| `dovetail-docking` | Dockable panels, tabs, split views |
| `dovetail-commands` | Commands, shortcuts, menus, context menus, command search |
| `dovetail-documents` | Document lifecycle, saving, recovery |
| `dovetail-theme` | Styling contracts and theme support |
| `dovetail-cli` | Project creation, development, build orchestration (installs the `dovetail` command) |

## First subset

[M00](../milestones/overviews/M00-workspace-foundation.md) creates only the crates in the [repository layout](overview.md#repository-layout): `dovetail-ui`, `dovetail-core`, `dovetail-macros`, `dovetail-layout`, `dovetail-render`, `dovetail-desktop`, `dovetail-controls` and `dovetail-cli`.

The other packages are added in the milestone that first needs them. Each addition is recorded in that milestone's report.

## Dependency graph

An arrow means "depends on".

```text
dovetail-ui (facade) ──► dovetail-controls ──► dovetail-core
        ├──► dovetail-desktop ──► dovetail-render ──► dovetail-layout ──► dovetail-core
        │            └──► dovetail-session ──► dovetail-layout
        ├──► dovetail-macros  (procedural macros, no runtime dependencies)
        └──► dovetail-core

dovetail-cli  (stand-alone tool, does not link the runtime)
```

External foundations:

- `dovetail-desktop` uses winit and AccessKit (`accesskit_winit`, [D0015](../decisions/D0015-accesskit-winit-adapter.md)). `dovetail-core` describes roles, names and states in its own types, so it keeps no dependencies.
- `dovetail-render` uses wgpu, and re-exports it (`dovetail::render::wgpu`) so canvases draw with exactly the version Dovetail pins (M09).
- From M04, `dovetail-render` also depends on `dovetail-layout`, because it paints the boxes that layout places.
- `dovetail-controls` does not depend on `dovetail-layout`: controls make elements, and layout places them. Only its tests use layout. Rust 1.100's check for unused dependencies found this, and that docking no longer used controls, on 2026-11-25.
- From M06, `dovetail-text` (fonts, rasterising, fallback and shaping) depends on nothing, and `dovetail-render` uses it to draw and measure text.
- From M06, `dovetail-controls` also depends on `dovetail-text`, for `TextField`'s text model (`TextBuffer`) and for moving the caret between lines.
- From M14, `dovetail-theme` depends on `dovetail-core` and `dovetail-macros`, and the facade re-exports it as `dovetail::theme`. `dovetail-controls`, `dovetail-commands`, `dovetail-docking` and `dovetail-documents` depend on it. The theme reaches the desktop and the painter through `Element::window_style`, so it does not depend on them.
- From M11, `dovetail-documents` depends on `dovetail-core`, `dovetail-controls`, `dovetail-commands` and `dovetail-macros`, and the facade re-exports it as `dovetail::documents`. File dialogs come through `dovetail-core`'s `FileDialogs` host, and window titles and close requests through elements, so the documents crate does not depend on the desktop crate either.
- From M10, `dovetail-docking` depends on `dovetail-core`, `dovetail-macros` and, from M14, `dovetail-theme`, and the facade re-exports it as `dovetail::docking`. Floating windows are opened through `dovetail-core`'s window host, which `dovetail-desktop` installs, so the docking crate does not depend on the desktop crate either.
- From M16, `dovetail-data` depends on `dovetail-core`, `dovetail-controls`, `dovetail-commands` (for `History`, D0022), `dovetail-macros` and `dovetail-theme`, and the facade re-exports it as `dovetail::data`. It learns where its views are scrolled to through `Element::on_scroll`, so it does not depend on the layout or desktop crates either.
- From M17, `dovetail-image` (decoders, animation, icons, the icon set: D0024) depends only on `dovetail-core`, for `Picture` and `View`. The facade re-exports it as `dovetail::image`. `dovetail-render` uses it to draw pictures, and `dovetail-controls`, `dovetail-commands` and `dovetail-data` use it for icons. Core knows a picture only by its size, fit, colour and label; what it draws is a value only the renderer opens.
- From M20, `dovetail-session` depends on `dovetail-core` and `dovetail-layout` only, with no winit, wgpu or AccessKit ([D0027](../decisions/D0027-testing-foundations.md)). It holds what `dovetail-desktop` used to keep for each window: the component tree, layout, focus, hover, presses, scroll bars, the caret blink, and size and scroll reports. `dovetail-desktop` depends on it and gives each window a `Session`. The facade does not re-export it: tests reach it through `dovetail-testing`.
- From M20, `dovetail-testing` depends on `dovetail-core`, `dovetail-layout` and `dovetail-session`. Applications add it as a dev-dependency; the facade does not re-export it, so test code stays out of applications ([D0027](../decisions/D0027-testing-foundations.md)). Its `pictures` feature adds `dovetail-render` and `dovetail-image`, to compare pictures, and its `windows` feature adds `dovetail-desktop`, for real-window tests (D0026). `dovetail-controls` uses it in its tests, and the counter and form examples in their self-tests. No framework package depends on it outside its tests.
- From M18, `dovetail-core` holds the system's settings (`SystemSettings`), frame requests and animated values (`use_animated`, `Transition`, `Animate`), with only the standard library ([D0028](../decisions/D0028-system-settings-and-animation.md)). `dovetail-desktop` reads the settings on each system and passes the session its share. `dovetail-layout` works out `Style::offset` and `Style::opacity`, and `dovetail-render` multiplies alpha by each box's opacity. `dovetail-theme` scales its text by the text scale, fades its colours, and has a `motion` group for the controls' transitions.
- From M08, `dovetail-commands` depends on `dovetail-core`, `dovetail-controls` and `dovetail-macros`, and the facade re-exports it as `dovetail::commands`. The system menu bar is reached through `dovetail-core`'s `NativeMenus` host, which `dovetail-desktop` installs, so the commands crate does not depend on the desktop crate.

## Rules

- `dovetail-core` depends on no other Dovetail crate.
- `dovetail-macros` creates code that calls `dovetail-core`, but does not link against it at run time.
- Concrete widgets live outside `dovetail-core`.
- A new package needs a reason in a milestone plan.
