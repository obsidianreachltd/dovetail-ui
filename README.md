# Dovetail UI

**A Rust framework for desktop applications on Windows, Linux and macOS.**

Dovetail UI is for desktop software of every kind: small tools and utilities, dashboards, creative and engineering software, and large editors that open and save documents. You write components in Rust with a declarative `ui!` macro, and Dovetail does the rest: layout, GPU painting, text, input, accessibility and the behaviour people expect from a desktop application.

> [!WARNING]
> **Dovetail UI is in early development and has not been released.** The crates are not on crates.io yet, and the API will change. Most features have been built and checked by hand on Windows only. On Linux and macOS, CI runs the tests and some example programs, but nothing has been checked by hand there yet. See [Status](#status).

## A first look

```rust
use dovetail::prelude::*;

#[component]
fn Counter() -> View {
  let count = use_state(|| 0);

  ui! {
    Column {
      Text { value: count.map(|value| format!("Count: {value}")) }
      Button {
        text: "Increment",
        on_click: move |_| count.update(|value| *value += 1),
      }
    }
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  dovetail::run("Counter", ui! { Counter {} })?;
  Ok(())
}
```

`use_state` makes a piece of state. When it changes, only the components that read it are built again. See the [syntax](docs/architecture/syntax.md) for the rest of the macro.

## Features

- **Components and state:** declarative components, reactive state, typed events and keyed lists, with errors from the macro that point at your code.
- **Our own layout and painting:** rows, columns, stacks, scrolling and layers, painted with the GPU through wgpu and sharp at every display scale.
- **Text:** TrueType and OpenType fonts, system fonts with fallback, kerning, editing, selection, the clipboard and input methods. Arabic, Hebrew and text that mixes directions are shaped and laid out correctly.
- **Accessibility:** every control reports a role and a label to screen readers through AccessKit. Keyboard focus works everywhere.
- **Controls:** buttons, toggles, text fields, check boxes, radio buttons, drop-downs, number fields, sliders, tooltips, tabs, dialogs, progress bars and more.
- **Data views:** lists, trees and tables that make only the rows in sight, so 100,000 rows scroll smoothly, and a property grid.
- **Commands and undo:** define a command once and use it from menus (native ones on macOS), toolbars, shortcuts, context menus and command search, with undo and redo.
- **Themes:** colours, text styles, shapes and spacing from a theme, made from one seed colour, with light, dark and high contrast that follow the system.
- **Icons and images:** PNG, JPEG and GIF, animated GIF and APNG, and a built-in icon set drawn from vector outlines in the theme's colours.
- **For large editors**, in packages of their own:
  - a GPU canvas that draws with your own wgpu pipeline, with pan, zoom and selection;
  - dockable panels with tabs, splits and floating windows, and layouts that are saved between runs;
  - documents with saving, opening, unsaved-change questions and crash recovery.
- **Testing tools:** tests that run components with no window, find elements by role and label as screen readers do, click and type through the window, and move time. They can also compare pictures and drive real windows. See [testing](docs/architecture/testing.md).
- **A command-line tool:** `dovetail new`, `dovetail dev` (rebuilds and restarts on save) and `dovetail build`.

Applications add only the packages they need. A small utility doesn't carry docking or documents.

## Getting started

You need [Rust](https://rustup.rs) 1.89 or later, and a graphics driver that supports Vulkan, DirectX 12 or Metal. Inside this repository, rustup uses the version pinned in `rust-toolchain.toml`.

Until the first release, Dovetail is used from a copy of this repository. Clone it, then install the `dovetail` command from it:

```sh
cargo install --path crates/dovetail-cli
```

Create an application and run it:

```sh
dovetail new my-app
cd my-app
dovetail dev
```

`dovetail dev` builds and runs the application, then rebuilds and restarts it whenever you save a source file. New projects use the framework from your copy of the repository, so keep it where it is.

`dovetail build --release` makes a release build and prints where it is.

## Examples

Run any example from the repository root with `cargo run -p <name>`. Most also take `--self-test`, which drives the window by itself and checks the result.

| Example | What it shows |
|---|---|
| `example-counter` | The counter above |
| `example-form` | A form with labelled fields, a toggle and checks on what is typed |
| `example-text-field` | Text fields on one line and on several |
| `example-controls` | Every everyday control, in light and dark |
| `example-themes` | One window in the light, dark, a custom and the system's theme |
| `example-commands` | Commands in menus, a toolbar, shortcuts and command search, with undo |
| `example-data` | A list, a tree and a table of 100,000 rows each, and a property grid |
| `example-images` | Images, icons in every control that takes them, and animations |
| `example-motion` | The system's settings as they change, and controls that move unless less motion is set |
| `example-gpu-canvas` | A PCB-like scene drawn with its own wgpu pipeline, with pan, zoom and selection |
| `example-docking` | An editor-like window with dockable and floating panels |
| `example-multi-window` | Several windows that remember their places, a modal settings window, and full screen |
| `example-documents` | A small board of parts kept as a document, with saving and recovery |
| `example-window` | The smallest program: a window cleared with wgpu |

## Packages

| Package | What it does |
|---|---|
| `dovetail-ui` | The package applications depend on. Its library is called `dovetail` and re-exports the rest. |
| `dovetail-core` | Components, state, events and the view tree. It has no dependencies. |
| `dovetail-macros` | `#[component]` and `ui!` |
| `dovetail-layout` | Layout, scrolling, focus and hit testing |
| `dovetail-session` | One window's components, layout, focus and input, with no window, so tests can run them |
| `dovetail-testing` | Tests for applications: find elements by role and label, click and type through the window, and move time |
| `dovetail-render` | GPU painting with wgpu, and custom canvases |
| `dovetail-desktop` | Windows, input, the clipboard, file dialogs and accessibility, with winit and AccessKit |
| `dovetail-text` | Fonts, shaping, bidirectional text and text editing |
| `dovetail-theme` | Themes and colour schemes |
| `dovetail-controls` | Everyday controls |
| `dovetail-commands` | Commands, shortcuts, menus, toolbars and undo |
| `dovetail-data` | Lists, trees, tables and the property grid |
| `dovetail-image` | Image decoding, animation, icons and the icon set |
| `dovetail-docking` | Dockable panels |
| `dovetail-documents` | Documents, saving and recovery |
| `dovetail-cli` | The `dovetail` command |

See [crates](docs/architecture/crates.md) for how they depend on each other.

## Design

- **The framework is the product.** A small PCB-like editor is built alongside it to prove it can carry a serious application ([D0001](docs/decisions/D0001-framework-is-the-product.md)).
- **Mostly our own code.** The only foundation libraries are [winit](https://github.com/rust-windowing/winit), [wgpu](https://github.com/gfx-rs/wgpu) and [AccessKit](https://github.com/AccessKit/accesskit). Layout, text, images and controls are written for Dovetail ([D0002](docs/decisions/D0002-build-own-stack.md)).
- **Desktop behaviour built in.** Focus, keyboard use, input methods, accessibility, commands and undo are part of the framework, not left to each application.
- **Decisions are written down.** Every design choice has a [decision record](docs/decisions/README.md), and every piece of work belongs to a [milestone](docs/milestones/README.md) with a plan and testable criteria.

## Status

Dovetail is built in milestones. The [completion matrix](docs/milestones/README.md) shows where each one stands.

- **Built and tested on Windows:** everything in [Features](#features).
- **Tested in CI on Linux and macOS:** the workspace tests, the GPU painting tests, and the window and counter examples.
- **Waiting:** checks by hand on Linux and macOS, such as with real screen readers, input methods and Wayland.
- **Before the first release:** system settings and animation, window management, testing tools for applications, files from the system, and localisation.
- **The first release** ([R01](docs/milestones/overviews/R01-first-release.md)): a validation editor that uses all of it, packages for each system, API documentation, and the crates published on crates.io.

## Documentation

- [Goals](docs/vision/goals.md) and [principles](docs/vision/principles.md)
- [Architecture](docs/architecture/overview.md) and [syntax](docs/architecture/syntax.md)
- [Milestones](docs/milestones/README.md), [decisions](docs/decisions/README.md) and [bugs](docs/bugs/README.md)
- [Glossary](docs/glossary.md)
- [All documentation](docs/README.md)

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) first: it explains how work is organised into milestones, the checks to run, and the writing style. AI coding agents should also read [AGENTS.md](AGENTS.md).

## Licence

Dovetail UI is owned by Obsidian Reach Ltd. You can use it under either the [MIT licence](LICENSE-MIT) or the [Apache 2.0 licence](LICENSE-APACHE), whichever you choose ([D0005](docs/decisions/D0005-permissive-licence.md)). That includes using it in closed-source and commercial products.

Unless you say otherwise, any contribution you submit for inclusion in Dovetail UI is licensed under both, with no extra terms.

Some data and test files come from other sources under their own terms. See [NOTICE](NOTICE).
