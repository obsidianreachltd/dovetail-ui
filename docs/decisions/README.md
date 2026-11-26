# Decisions

A decision record explains one important choice: why it was made, and what it means. Each decision has its own file, `D0001-short-name.md`, written from the [template](_template.md). Numbers are never reused.

## Decision list

| ID | Decision | Status | Date |
|---|---|---|---|
| [D0001](D0001-framework-is-the-product.md) | The framework is the product | Accepted | 2026-11-20 |
| [D0002](D0002-build-own-stack.md) | Build our own stack on a few foundations | Accepted | 2026-11-20 |
| [D0003](D0003-accesskit-for-accessibility.md) | Use AccessKit for accessibility | Accepted | 2026-11-20 |
| [D0004](D0004-cargo-workspace-monorepo.md) | One repository as a Cargo workspace | Accepted | 2026-11-20 |
| [D0005](D0005-permissive-licence.md) | Use a permissive licence | Accepted | 2026-11-20 |
| [D0006](D0006-facade-crate-name.md) | Name of the facade crate | Accepted | 2026-11-20 |
| [D0007](D0007-one-shared-gpu-device.md) | One GPU device shared by all windows | Replaced by D0009 | 2026-11-20 |
| [D0008](D0008-draw-on-demand.md) | Draw only when needed | Accepted | 2026-11-20 |
| [D0009](D0009-replace-surfaces-on-device-loss.md) | One shared GPU device, with surfaces replaced on device loss | Accepted | 2026-11-20 |
| [D0010](D0010-copy-arena-handles.md) | Reactive state uses Copy arena handles | Accepted | 2026-11-20 |
| [D0011](D0011-macro-dependencies.md) | Dependencies for the macros | Accepted | 2026-11-20 |
| [D0012](D0012-m04-text.md) | How M04 draws text | Accepted | 2026-11-20 |
| [D0013](D0013-scoped-unsafe-for-platform-code.md) | `unsafe` only in marked platform modules | Accepted | 2026-11-20 |
| [D0014](D0014-unicode-data.md) | Unicode tables generated from the Unicode Character Database | Accepted | 2026-11-20 |
| [D0015](D0015-accesskit-winit-adapter.md) | AccessKit through its winit adapter, pinned | Accepted | 2026-11-22 |
| [D0016](D0016-docking-foundations.md) | Docking: our own layout format, drag and drop in core | Accepted | 2026-11-22 |
| [D0017](D0017-documents-foundations.md) | Documents: native file dialogs, recovery snapshots, tasks in core | Accepted | 2026-11-23 |
| [D0018](D0018-complex-script-foundations.md) | Complex scripts: the full bidirectional algorithm, paragraph direction, one caret | Accepted | 2026-11-23 |
| [D0019](D0019-theming-foundations.md) | Theming: Flutter-like theme data, OKLCH colour schemes, values passed down the tree | Accepted | 2026-11-23 |
| [D0020](D0020-default-theme-look.md) | The default theme: deep slate, smaller text, bold buttons | Accepted | 2026-11-23 |
| [D0021](D0021-everyday-controls-foundations.md) | Everyday controls: scrollbars, context menus and tooltips | Accepted | 2026-11-23 |
| [D0022](D0022-data-views-foundations.md) | Data views: rows made from the scroll position, data from functions | Accepted | 2026-11-24 |
| [D0023](D0023-images-and-icons.md) | Images and icons: our own decoders, SVG outlines, a texture for each image | Replaced by D0024 | 2026-11-24 |
| [D0024](D0024-images-icons-and-animation.md) | Images, icons and animation: our own decoders, SVG outlines in their colours, a texture for each image | Accepted | 2026-11-24 |
| [D0025](D0025-repaint-into-a-kept-texture.md) | Windows paint into a texture kept between frames | Accepted | 2026-11-25 |
| [D0026](D0026-real-window-tests.md) | Real-window tests run from `cargo test`, driving the event loop step by step | Accepted | 2026-11-25 |
| [D0027](D0027-testing-foundations.md) | Testing foundations: windowless input in `dovetail-session`, a clock in core, and `dovetail-testing` | Accepted | 2026-11-25 |
| [D0028](D0028-system-settings-and-animation.md) | System settings in core, animation on frames, and opacity and offsets in layout | Accepted | 2026-11-26 |

## Status words

| Status | Meaning |
|---|---|
| Proposed | Written down, but not yet agreed. |
| Accepted | Agreed. The work follows it. |
| Replaced | No longer followed. The record links to the decision that replaces it. |

## Making a decision

1. Copy the template to a new file with the next number, with status Proposed.
2. Add a row to the list above.
3. When it is agreed, change the status to Accepted and update the date in both places.
4. Do not edit an Accepted decision to change it. Write a new one, and mark the old one as Replaced.
