# D0019 — Theming: Flutter-like theme data, OKLCH colour schemes, values passed down the tree

| Field | Value |
|---|---|
| ID | D0019 |
| Status | Accepted |
| Date | 2026-11-23 |

## Context

M14 adds themes. Several choices shape the work:

- what a theme is made of, and how components find it;
- how a colour scheme is made from one seed colour;
- whether themes follow the system's accent colour;
- how bold text is drawn where the system has no bold font file.

## Decision

- **Values are passed down the component tree in `dovetail-core`.** A component provides a value with `provide_context`, and any component inside it reads the nearest one with `use_context`, by its Rust type. Themes are passed as a signal, so only the components that read the theme are built again when it changes. A window that a component opens, such as a floating panel, sees the values of the component that opened it.
- **A theme is a Rust struct, as in Flutter.** `ThemeData` has a colour scheme, a text theme, shapes, spacing, an optional theme for each kind of control, and typed extensions that applications add for their own widgets. The `Theme` component provides it; `theme()` reads it.
- **Colour schemes are made from a seed colour with tonal palettes in OKLCH.** Each colour role takes a tone of the seed's hue. Text roles are then checked against their backgrounds and moved towards black or white until they meet WCAG AA contrast (4.5 to 1).
- **The default theme uses Dovetail's own blue.** Applications that want the system's accent colour ask for it, with `system_appearance().accent`.
- **Bold and other weights come from the system's font files, and bold is made by thickening outlines where there is no file**, as browsers do.
- **The theme reaches the painter through the window's outermost themed element** (`Element::window_style`): the background, the default text colour, the focus ring and selection colours, and whether the title bar is dark.

## Consequences

- Controls in every package look the same and change together, and an application changes them in one place.
- Any component can pass values down, not only themes.
- Colours are close to Flutter's for the same seed, but not identical.
- Text style now has a weight as well as a size, so layout's `Measure` takes a `TextStyle`, not only a size.

## Alternatives considered

- **An open table of named tokens:** flexible, and easy to load from a file, but typos are found only at run time.
- **Material's HCT colour space:** the same colours as Flutter, but several hundred lines of colour science.
- **Following the system accent by default:** native-looking, but applications would look different on every machine.
- **Only weights with font files:** simpler, but bold text would show as regular on macOS, whose system font is a variable font Dovetail cannot read yet.
