# M14 — Theming

| Field | Value |
|---|---|
| ID | M14 |
| Status | In progress |
| Depends on | M06, M10, M11 |
| Plan | [M14 plan](../plans/M14-theming.md) |
| Report | — |

## Summary

Build `dovetail-theme`: a theme that holds an application's colours, text styles, shapes and spacing, passed down the component tree and read by every control. It works like Flutter's `ThemeData` and `Theme.of(context)`.

## Why it matters

- **Fixed colours:** every control has a fixed look. Its colours are constants in `dovetail-controls`, `dovetail-commands`, `dovetail-docking` and `dovetail-documents`: 13 of them today.
- **What applications need:**
  - their own colours and text;
  - light and dark modes that follow the system;
  - high contrast for people who need it.
- **The first release:** applications of every kind need this (see [goals](../../vision/goals.md)), so it comes before [R01](R01-first-release.md).
- **Later work:** every widget added after this milestone takes its look from the theme.

## What it includes

- **Values passed down the tree**, in `dovetail-core`:
  - a component provides a value to everything inside it, and any component below reads the nearest one, as Flutter's `InheritedWidget` does;
  - a component that reads it is built again when it changes, and only those components;
  - this is general, not only for themes.
- **A theme**, made of:
  - a colour scheme of named roles, such as primary and the colour on primary, surface and the colour on surface, outline, and error;
  - a text theme of named styles, such as title, body and label, each with a size, a weight and a line height;
  - shapes (corner radii), spacing and density.
- **Colour schemes from one seed colour**, as Flutter's `ColorScheme.fromSeed` does, with text that can be read against each background.
- **Component themes**, such as a button theme, a text field theme and a tab theme. Each overrides the look of one kind of control, and falls back to the colour scheme and text theme.
- **A `Theme` component** that overrides all or part of the theme for the components inside it.
- **Light and dark themes:**
  - they follow the system setting, and change while the application runs;
  - the system's accent colour and high-contrast setting are used where the system gives them.
- **Bold and other weights** of system fonts, found and used by `dovetail-text`, so text styles can have weights.
- **Every built-in control uses the theme:** those in `dovetail-controls`, `dovetail-commands`, `dovetail-docking` and `dovetail-documents`, and the focus ring and selection colours in the painter.
- **An example** that shows the same window in the light, dark and a custom theme.

## What it leaves out

- Flutter's Material Design look and widgets. Dovetail takes the way Flutter's themes work, not its design language.
- Animating from one theme to another, which needs an animation system (after R01).
- The native look of each system, such as Fluent on Windows or Aqua on macOS.
- Style sheets in a separate language, such as CSS.
- A tool for designing themes.

## Done when

1. A component reads the nearest theme from the tree, without being passed it, and is built again when the theme changes.
2. Every built-in control takes its colours, text styles and shapes from the theme, and no fixed colours are left in the control packages.
3. A `Theme` component overrides part of the theme for what is inside it, and nothing outside it changes.
4. Switching the system between light and dark changes a running application's theme on Windows, Linux and macOS.
5. A colour scheme made from a seed colour gives text with at least WCAG AA contrast (4.5 to 1) on every background it is meant for, checked by a test.
6. The example shows the light, dark and a custom theme, and checks each with a self-test.
