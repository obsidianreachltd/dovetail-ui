# M15 plan — Everyday controls

| Field | Value |
|---|---|
| ID | M15 |
| Status | In progress |
| Overview | [M15 overview](../overviews/M15-everyday-controls.md) |
| Last updated | 2026-11-23 |

## Goal

Add the controls almost every desktop application needs and Dovetail does not have yet: scrollbars, check boxes, radio buttons, drop-downs, number fields, sliders, tooltips, context menus, and a few smaller pieces.

## Starting point

- **Controls:** `Button`, `Toggle`, `TextField`, `ProgressBar` and `Dialog` in `dovetail-controls`; `MenuBar`, `Toolbar`, `CommandButton` and `CommandSearch` in `dovetail-commands`. All read `theme()`.
- **Scrolling:** layout keeps each `Style::scroll` element's offset and how far it can go (`scroll.rs`), and the wheel moves it. Nothing shows that an element scrolls.
- **Layers:** `Layer::Below`, `Layer::At` and `Layer::Fill` place menus and backdrops above the window, and are hit first.
- **Input:** presses, the pointer with capture, the wheel, keys and focus. There is no hover: elements are not told when the pointer is over them.
- **Accessibility:** roles up to `Dialog`, `toggled` (on or off only), and `value` from 0 to 1 for progress bars. There is no range, no mixed state, no description and no expanded state.
- **Tasks:** `Task::after` (M11) runs code after a pause, for tooltips.
- **Decided on 2026-11-23** ([D0021](../../decisions/D0021-everyday-controls-foundations.md)):
  - layout and the painter draw scrollbars;
  - bars float, thin, over the content's edge;
  - context menus are drawn by Dovetail;
  - tooltips are a wrapper component.

## Steps

1. [x] Settle the open questions (above), on 2026-11-23.
2. [x] **Core**, done on 2026-11-23:
   - `Element::on_hover` (the pointer came onto or left the element, or something inside it; a layer counts as inside its element) and `Element::on_focus_within`;
   - `Element::description`, which assistive tools read. An element with no role gives it to the first element inside that has one;
   - `Element::expanded`, `Element::mixed` and `Element::text_value`, and `Element::range` with `value_in` and `ValueRange`, for values in any range;
   - `Element::on_context_menu`, asked to open a menu at a point;
   - roles: radio button and group, combo box, spin button, slider, tooltip, link, status bar and separator. AccessKit's splitter stands for the separator, as in ARIA;
   - `AccessAction::Increment`, `Decrement` and `SetNumber`, and `NamedKey::ContextMenu`;
   - `Style::scrollbars`, true by default, and scrollbar colours in `WindowStyle`.
3. [x] **Scrollbars**, done on 2026-11-23:
   - layout works out each bar's track and thumb (`Layout::scrollbars`). It finds the bar under the pointer, but not one under a layer (`scrollbar_at`), and scrolls to a place (`scroll_to`);
   - the painter draws each bar right after the last box inside its element, so it is over the content and under layers. It is 4 pixels wide, or 12 with its track while under the pointer or dragged;
   - the window drags a thumb, and moves one page on a click in the track. On macOS, with the overlay setting, bars show for a second after scrolling and while under the pointer;
   - assistive tools read the scroll position and its range.
4. [x] **Desktop input**, done on 2026-11-23:
   - hover, checked after every pointer move and every layout; only elements whose state changed are told;
   - focus within;
   - a right-click asks the nearest `on_context_menu`. The Menu key and Shift+F10 ask the one around the focused element, and the menu opens below that element;
   - Expand and Collapse from assistive tools press an element that opens and closes.
5. [x] **Controls**, done on 2026-11-23, each with a role, name, value and state, a disabled state, and the theme's looks:
   - `CheckBox` (on, off, mixed);
   - `RadioGroup`: only the chosen button is a Tab stop, and the arrow keys, Home and End move the choice and focus;
   - `Select`: a list in a layer. While it is closed, the arrows and letters change the choice at once, as on Windows;
   - `NumberField`: a text field with a range and a step. The buttons, the arrows, Page Up and Page Down change it, and so does the wheel while it has focus;
   - `Slider`;
   - `Tooltip`: shows after `TOOLTIP_DELAY` (500 ms) on hover, and at once on focus. Escape hides it;
   - `StatusBar`, `Tabs`, `Collapsible`, `Separator` and `Link`.
6. [x] **Context menus**, done on 2026-11-23: `ContextMenu` in `dovetail-commands`, with the menu bar's `MenuItem`s and rows.
7. [x] **Theme**, done on 2026-11-23:
   - `CheckTheme`, `SliderTheme` and `TooltipTheme`, with `check_look`, `slider_look` and `tooltip_look`;
   - scrollbars take the outline colour, on a high container.
8. [x] **`examples/controls`**, done on 2026-11-23:
   - every control, a scrolling list with a context menu, and a status bar, in light and dark;
   - a `--self-test` of 27 steps.
9. [x] **Tests**, done on 2026-11-23:
   - layout: scrollbar geometry, `scroll_to` and `scrollbar_at` (6); stretching in a column that fits its content (1);
   - controls: every control with the pointer and the keyboard, and the tooltip on focus and after its pause (9);
   - commands: the context menu from a right-click, with shortcuts, greyed-out items and keys (3);
   - desktop: the AccessKit tree for every new role, value, range, state, description and action (1), and the scroll position (1);
   - render, on the GPU: a thin and a wide scrollbar (1).

## Prototypes and open decisions

- **Scrollbars:** decided, drawn by layout and the painter (D0021).
- **Bar space:** decided, floating and thin (D0021).
- **Context menus:** decided, drawn by Dovetail (D0021).
- **Tooltips:** decided, a wrapper component (D0021).
- **Right-clicks reach a context menu through `Element::on_context_menu`**, not through the pointer handler. Only the nearest element with a pointer handler gets the pointer. So a right-click on a text field inside a `ContextMenu` would not reach the menu.
- **Stretching**, fixed on 2026-11-23:
  - A column with `Align::Stretch` stretched its children only when its own width was known first.
  - A list or menu as wide as its content, with a smallest width, therefore left its rows narrow, and highlights did not fill the row.
  - Layout now stretches the children once the width is known. This also fixes the menu bar's menus.
- **A press on a radio button or a tab moves focus to it.** Only the chosen one is a Tab stop, so a press would otherwise leave focus where it was. The self-test found this.
- **Follow-ups, after M15:**
  - `Align::Stretch` overrides fixed sizes across, unlike CSS. Changing that would touch every layout;
  - an underline for links, which needs text decoration in the painter;
  - drawing the check mark as a shape rather than the "✓" character, with M17's icons;
  - [B0006](../../bugs/B0006-text-field-right-walk-flaky.md): the text field self-test's Right walk sometimes ends early.

## Risks

- **Hover is new input.** It runs on every pointer move, so it walks only the boxes under the pointer, and tells only elements whose state changed.
- **Bars over content** can cover the last pixels of a row. They are thin, and wide only under the pointer.
- **macOS's scroll bar setting** is read from `NSScroller` and has not been run here: it is type-checked, and must be run on a Mac.

## Testing

- **Unit tests** in layout for scrollbar geometry and `scroll_to`, and in controls for each control's keyboard and pointer behaviour, built in a tree and driven through its handlers.
- **AccessKit tree tests** in the desktop for every new role, range, mixed and expanded state, and description.
- **`examples/controls --self-test`** in a real window: dragging a scrollbar, a track click, every control with the pointer and the keyboard, a tooltip on hover and on focus, and a context menu with a greyed-out item, in light and dark.
- **By hand, on each system:** the look of the bars, and macOS's scroll bar setting.
- **Results on 2026-11-23**, Windows 11:
  - 278 workspace tests pass, every GPU test passes, and clippy is clean, including for macOS and Linux;
  - `examples/controls --self-test` passes all 27 steps, and every other example's self-test passes. The text field's self-test failed once, in a step M15 does not touch, and passed twice after ([B0006](../../bugs/B0006-text-field-right-walk-flaky.md));
  - the real example, driven through UI Automation only, with no keys or mouse input: pictures in light and dark look right, and the drop-down opens from UI Automation's Expand.

## Done when

Copied from the overview.

1. Every scrolling element shows a scrollbar that can be dragged, and a click on its track moves one page.
2. Check boxes, radio groups, drop-downs, number fields and sliders can each be used with the pointer and with the keyboard alone.
3. A tooltip shows on hover and on focus, and assistive tools read it.
4. A right-click opens a context menu of commands, with their shortcuts, and greyed-out items cannot be chosen.
5. Every new control has the right AccessKit role and state, checked by tests on the AccessKit tree.
6. The example shows every control in the light and dark themes, and its self-test uses each one.
