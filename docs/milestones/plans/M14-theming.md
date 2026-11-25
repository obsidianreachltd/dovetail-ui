# M14 plan — Theming

| Field | Value |
|---|---|
| ID | M14 |
| Status | In progress |
| Overview | [M14 overview](../overviews/M14-theming.md) |
| Last updated | 2026-11-23 |

## Goal

Build `dovetail-theme`: a theme that holds an application's colours, text styles, shapes and spacing, passed down the component tree and read by every control.

## Starting point

- **Colours are constants** in each package: `ACCENT`, `MUTED` and `DISABLED` in `dovetail-controls`, and others in `dovetail-commands`, `dovetail-docking`, `dovetail-documents` and the painter (focus ring and selection). There are 46 uses of fixed colours.
- **Style inheritance:** text size and colour are passed down by layout (`Style::font_size`, `Style::text_color`), with black text by default. There is no font weight.
- **No way to pass values down:** core has no way for a component to give a value to the components inside it. Each component's parent is known only to `Tree`.
- **Windows:** windows read their title and close guard from their elements after layout (M11), which the theme's window style can follow.
- **System appearance:** winit reports light or dark on Windows and macOS (`Window::theme`, `WindowEvent::ThemeChanged`), but not the accent colour or high contrast, and not dark mode on X11.
- **Decided on 2026-11-23** ([D0019](../../decisions/D0019-theming-foundations.md)):
  - a Flutter-like `ThemeData` struct with typed extensions;
  - OKLCH tonal palettes from a seed colour;
  - the system accent only when the application asks;
  - weights from font files, then synthetic bold.

## Steps

1. [x] Settle the open questions (above), on 2026-11-23.
2. [x] **Core**, done on 2026-11-23:
   - `provide_context`, `use_context` and `context_from` in `tree.rs`, with each component's parent kept in a table of its own on the UI thread; `current_component`; `Tree::mount_within`;
   - `WindowRequest::context`: docking's floating windows see the dock area's values, so they have its theme;
   - `FontWeight` and `Style::font_weight`, inherited like the font size;
   - `Element::window_style` with `WindowStyle` (background, focus ring, selection colours, dark);
   - `SystemAppearance`, read with `system_appearance`, which marks the readers to be built again when `set_system_appearance` changes it.
3. [x] **Text weights**, done on 2026-11-23:
   - `Fonts::system_weight` puts the nearest weight's font file first (Segoe UI Light to Black on Windows, DejaVu Sans Bold on Linux), choosing a heavier file first for bold weights, as CSS does;
   - where the nearest file is lighter than a bold weight asks for, glyphs are thickened (`embolden`), half on each side, without moving the pen: always on macOS, whose system font is a variable font;
   - layout's `Measure` takes a `TextStyle` (size and weight), and `Content::Text` has a weight;
   - the painter draws each weight with its own fonts, with the weight in the glyph atlas key.
4. [x] **`dovetail-theme`**, done on 2026-11-23:
   - `color.rs`: sRGB, OKLab and OKLCH, keeping colours inside what a screen can show by lowering chroma, tones as CIE L*, and WCAG contrast;
   - `ColorScheme` with 20 roles and `from_seed`: tonal palettes of the seed's hue for primary, secondary, neutral, neutral variant and error, with Material 3's tones, and text moved until it has 4.5 to 1 contrast. `high_contrast` makes text black or white;
   - `TextTheme`, `Shapes`, `Spacing` (with density), component themes for buttons, switches, text fields, menus, tabs and dialogs, and `Extensions`;
   - `ThemeData::light`, `dark`, `from_seed`, `high_contrast`, `with_extension`, and resolved looks for controls;
   - the `Theme` component, with `ThemeMode` (system, light, dark), and `theme()`. The outermost `Theme` fills its space with the background, gives the body text style and colour to what is inside, and sets the window style.
5. [x] **System appearance in the desktop**, done on 2026-11-23:
   - Windows: dark from `AppsUseLightTheme`, the accent from the window manager's `AccentColor`, high contrast from `SystemParametersInfoW`;
   - macOS: `AppleInterfaceStyle`, `NSColor.controlAccentColor` in sRGB, and `NSWorkspace`'s contrast setting;
   - Linux: GNOME's `color-scheme`, `accent-color` and `high-contrast` through `gsettings` (`appearance.rs`, with tests of the parsing);
   - read at start, when winit says the system theme changed, and when a window comes to the front;
   - each window reads its style after layout. The painter takes the background, focus ring and selection colours from it (`PaintInput::colors`), and the title bar is dark when it is.
6. [x] **Every control uses the theme**, done on 2026-11-23: buttons, switches, text fields, the progress bar and dialogs in `dovetail-controls`; menus and command search in `dovetail-commands`; tabs, splitters and drop previews in `dovetail-docking`; the file dialog in `dovetail-documents`. `ACCENT`, `MUTED` and `DISABLED` are gone.
7. [x] **`examples/themes`**, done on 2026-11-23: light, dark, a custom green theme with rounder, roomier controls, and the system's theme, chosen with buttons. A section overrides the primary colour with the error colour, and a badge reads a theme extension.
8. [x] **Tests**, done on 2026-11-23:
   - colour maths (4), and contrast of every text role for 99 seeds, light and dark, each also in high contrast (2);
   - themes in a component tree (4): the nearest theme, changes reaching only readers, an override that changes only its subtree, the system's dark mode and high contrast, and values seen from a second tree;
   - a scan of the four control packages for fixed colours;
   - weights (2): Segoe UI Bold from its file, and synthetic bold;
   - GNOME's settings (1);
   - `examples/themes --self-test` (8 checks): the window style and every control's colours in each theme, the override, and the system mode following dark, light and high contrast set from inside the program.

## Prototypes and open decisions

- **Theme data:** decided, a Flutter-like struct (D0019).
- **Seed colours:** decided, OKLCH tonal palettes (D0019).
- **Accent:** decided, only when asked (D0019).
- **Weights:** decided, files then synthetic bold (D0019).
- **Plain text in a nested `Theme`:** controls follow the nearest theme, but plain text takes its colour from the outermost one, because a nested `Theme` adds no element of its own.
- **The default look**, changed on 2026-11-23 after seeing it ([D0020](../../decisions/D0020-default-theme-look.md)): a deep slate seed kept as the light primary, 14-pixel body text, and bold button labels.
- **High contrast is applied once**, by the outermost `Theme`. A nested theme is built from `theme()`, which has it already. The self-test found this.
- **Follow-ups, after M14:**
  - reading variable fonts by weight, so macOS shows its own bold rather than thickened glyphs;
  - a theme's text styles for plain text inside a nested `Theme`;
  - animating from one theme to another (after R01, as the overview says).

## Risks

- **Changing `Measure`** touches layout, the painter, AccessKit and their tests. It is mechanical, and the tests show if anything moved.
- **Colours change a little** from today's constants, because the default theme is made from a seed. Pictures in the docs may look slightly different.
- **System settings on Linux** come from `gsettings`, which only GNOME and desktops like it have. Elsewhere, light is assumed.
- **macOS and Linux code not run here:** it is type-checked, and must be run on those systems.

## Testing

- **Unit tests** for passing values down, colour conversion, schemes and contrast, and weights.
- **`examples/themes --self-test`** for the themes in a real window, switching between them, and the window style.
- **By hand, on each system:** switching the system between light and dark while the example runs. Not done yet.
- **Results on 2026-11-23**, Windows 11:
  - 256 workspace tests pass, every GPU test passes, and every example's self-test passes;
  - the real example, driven through UI Automation only, with no keys or mouse input: pictures of the light, dark, custom and system themes look right. The dark theme has a dark title bar. The system theme came out dark, because this machine is set to dark mode, so reading the system works. Headings use Segoe UI Semibold.

## Done when

Copied from the overview.

1. A component reads the nearest theme from the tree, without being passed it, and is built again when the theme changes.
2. Every built-in control takes its colours, text styles and shapes from the theme, and no fixed colours are left in the control packages.
3. A `Theme` component overrides part of the theme for what is inside it, and nothing outside it changes.
4. Switching the system between light and dark changes a running application's theme on Windows, Linux and macOS.
5. A colour scheme made from a seed colour gives text with at least WCAG AA contrast (4.5 to 1) on every background it is meant for, checked by a test.
6. The example shows the light, dark and a custom theme, and checks each with a self-test.
