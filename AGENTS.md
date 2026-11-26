# AGENTS.md

Instructions for AI coding agents working in this repository. Human contributors should read [CONTRIBUTING.md](CONTRIBUTING.md). The same rules apply to both.

## The project

Dovetail UI is a Rust framework for desktop applications of every kind on Windows, Linux and macOS. It provides a `dovetail` command, a declarative `ui!` macro, and optional packages for controls, text, data views, images, theming, commands, docking and documents. A small PCB-like editor is built alongside it to prove that it works.

Before you change anything, read the [goals](docs/vision/goals.md), the [principles](docs/vision/principles.md) and the [architecture overview](docs/architecture/overview.md).

## Where things stand

- The [completion matrix](docs/milestones/README.md) shows each milestone's status. Work on the milestones that are In progress or Planned.
- Each milestone's plan says what is built, what is left, and the follow-ups. Read the plan before you work on a milestone. Its results and decisions are there, not here.
- Most work has been built and checked by hand on Windows only. CI runs the tests on Linux and macOS, but most features have not been used by hand there.

## Decisions you must respect

- The framework is the product, not the PCB editor ([D0001](docs/decisions/D0001-framework-is-the-product.md)).
- We build our own stack. The only foundation libraries are winit, wgpu and AccessKit ([D0002](docs/decisions/D0002-build-own-stack.md), [D0003](docs/decisions/D0003-accesskit-for-accessibility.md)).
- The project is one repository with a Cargo workspace ([D0004](docs/decisions/D0004-cargo-workspace-monorepo.md)).
- The licence is MIT OR Apache-2.0 ([D0005](docs/decisions/D0005-permissive-licence.md)). New dependencies must be compatible with it. Copyright lines appear only in `NOTICE` and `LICENSE-MIT`, not in source files.
- The facade package is `dovetail-ui`, with library name `dovetail` ([D0006](docs/decisions/D0006-facade-crate-name.md)). Do not rename it back.
- `dovetail-core` has no dependencies. Follow the other [dependency rules](docs/architecture/crates.md#rules).
- `unsafe` is allowed only in `crates/dovetail-desktop/src/platform/`, with a `// SAFETY:` comment on every block ([D0013](docs/decisions/D0013-scoped-unsafe-for-platform-code.md)).
- winit, wgpu, AccessKit and its winit adapter are pinned to exact versions in the root `Cargo.toml`. Upgrade them only as a planned step, together, because the adapter must match winit.
- `rust-toolchain.toml` pins the Rust version for building, linting and testing, and CI uses the same one. Upgrade it only as a planned step (see [CONTRIBUTING.md](CONTRIBUTING.md#setup)). The oldest supported version is `rust-version` in `Cargo.toml`, Rust 1.89: do not use features newer than it.

## Writing framework code

- **Generated code** from `#[component]` and `ui!` refers to `::dovetail`. A crate that uses `dovetail-core` directly adds `extern crate dovetail_core as dovetail;`.
- **State is made while a component builds**, with `use_state`. This includes framework objects such as `Commands`, `Dock`, `Document`, `Selection` and `Expansion`, for example `use_state(Commands::new)` or `use_state(|| Dock::new(..)).get()`.
- **Keep state in signals, not in views.** Views are built again often. Docking builds a panel's content again when the panel moves.
- **Edits go through `History`**, so they can be undone. Change a `Document` only with `edit`.
- **Colours come from the theme.** Controls read `theme()`. Never add a fixed colour to the control packages: a test scans them.
- **Accessibility:** give every control a role and every text field a `label`. A picture has a label, or is decoration.
- **Animate with `use_animated`**, not timers of your own, so less motion and tests reach it. Controls take their `Transition` from `theme().motion`. Animate how far a change has gone, from 0 to 1, and mix the theme's colours with it, so a theme that fades is not held back.
- **Applications use Dovetail's wgpu**, `dovetail::render::wgpu`, in canvases.
- **After a GPU device loss**, replace the surfaces as well as the device. Never reconfigure an old surface with a new device (see `Gpu::replacing`).
- **Unicode tables** are generated from the UCD by a tool ([D0014](docs/decisions/D0014-unicode-data.md)). Never edit them by hand.

## Testing

- **Self-tests:** every example has a `--self-test` that drives its own window. Add one to any new example, with `TestWindows`, and run every example's self-test after a change to the desktop, layout or painting code.
- **Test UIs with `dovetail-testing`** ([testing](docs/architecture/testing.md)): headless with `TestUi` first, finding elements by role and label. Move its clock with `advance`; never sleep for timers. UI-thread code reads the time with `dovetail_core::now()`.
- **Input in tests goes through the window** (`TestUi`, `TestWindows`, `Context::click`, `Context::key`), not through the system. A test that must send real key presses checks, before every key, that the window under test is in front, and stops if it is not.
- `DOVETAIL_LOG_INPUT=1` makes windows log every key, input-method and focus event to stderr ([B0004](docs/bugs/B0004-doubled-letter-when-typing.md)).
- The other test commands, and the test data that is not committed, are in [CONTRIBUTING.md](CONTRIBUTING.md#other-tests-and-tools).

## Repository layout

```text
README.md, AGENTS.md, CONTRIBUTING.md
Cargo.toml          workspace root
crates/             framework crates
.github/workflows/  CI on Windows, Linux and macOS
examples/           example applications
prototypes/         throw-away prototypes kept for their reports
tools/              development tools
docs/
  vision/           goals and principles
  architecture/     design documents
  decisions/        decision records (Dnnnn)
  milestones/       completion matrix; overviews/, plans/, reports/ (Mnn, Rnn)
  bugs/             bug records (Bnnnn)
  glossary.md
```

`Mnn` milestones build part of the framework, and `Rnn` milestones are releases. IDs are not the order of work: the matrix's "Depends on" column is.

## Commands

Run these at the workspace root before you finish any change:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

After a change to platform code, also type-check the other systems:

```sh
cargo clippy -p dovetail-desktop --target aarch64-apple-darwin -- -D warnings
cargo clippy -p dovetail-desktop --target x86_64-unknown-linux-gnu -- -D warnings
```

## Rules

1. **Work through milestones.** Only do work that belongs to the current milestone's plan. If you find other work, record it in the plan's Follow-up section or as a bug.
2. **Follow the templates.** A new overview, plan, report, decision or bug starts as a copy of its `_template.md`. Keep every heading and its order.
3. **Keep the status the same everywhere.** When a milestone changes, update its overview, its plan, its report and the [matrix](docs/milestones/README.md) together.
4. **Record decisions.** Any change to a decision, or any new dependency beyond winit, wgpu and AccessKit, needs a decision record first. Do not edit an Accepted decision. Replace it with a new one.
5. **Record bugs.** Use the [bug template](docs/bugs/_template.md) and add the bug to the [bug list](docs/bugs/README.md).
6. **Write plainly.** Use British English, short sentences, and technical terms only when they are needed. See the [writing style](docs/README.md#writing-style).
7. **Test on all three systems.** Changes must pass CI on Windows, Linux and macOS. Platform-specific work needs the manual checks from its plan.
8. **Do not commit generated code.** Code from macros is a build output.
9. **Keep this file short.** Put what a milestone built in its plan, not here. Add a line here only for a rule that holds across milestones.
