# D0030 — Command state, toolbar overflow by measuring, and pickers drawn with images

| Field | Value |
|---|---|
| ID | D0030 |
| Status | Accepted |
| Date | 2026-11-27 |

## Context

M25 adds toggle buttons, button groups, tool palettes, and date, time and colour pickers ([M25 plan](../milestones/plans/M25-pickers-and-tool-palettes.md), step 1). Its plan leaves six questions open:

- **Toolbar overflow:** the toolbar must know which buttons fit. Either layout hides the children that do not fit, or the toolbar measures them and builds again.
- **The colour area:** a square of colours mixed in two directions. The painter has no gradients.
- **Letter keys for tools:** a shortcut with no modifier, such as `B`, must not take letters typed into a text field.
- **Long press:** how long, and where it belongs.
- **Today's date:** the standard library has no local time. What do tests and headless runs get?
- **Command state and history:** whether turning a command on or off can be undone.

M25 also needs to say how a command holds its state, and which AccessKit roles the new controls use.

## Decision

### Command state lives in `Commands`

- **Three kinds of command:**
  - a plain command, as now;
  - a toggle, which is on or off: `Commands::define_toggle`. Running it turns it over;
  - a choice, one of a named group of which one is on: `Commands::define_choice`. Running it makes it the one that is on, and turns the others in its group off.
- **The state** is kept in the registry, so every place that shows the command follows it:
  - `set_on` sets it;
  - `on_when` gives a rule that is read where the command is shown, as `enabled_when` does;
  - `is_on` reads it, and `chosen` gives the command that is on in a group.
- **`CommandInfo`** says the state in `on: Option<bool>`, `None` for a plain command, and the group in `group: Option<String>`.
- **Not an edit:** turning a command over does not go through `History`. View options, such as a grid or a ruler, are not part of the document. A command whose state belongs to a document can still make an edit in what it runs.

### Toolbar overflow: the toolbar measures, in the same update

- `Toolbar` with `overflow` set fills the width it is given. It wraps each child in a box that reports its width with `on_size`, and keeps the widths in state.
- The session already builds and lays out again, in the same update, when a size it reports changes what is built (D0022). So the toolbar builds only the children that fit, and a "More" button at its end, with no frame shown in between.
- The More button opens a layer under it with the children that did not fit, as they are. Each can be run from there, and shows its state.
- Layout does not change. A toolbar without `overflow` is laid out as before.

### The colour area is an image

- The picker draws its square of saturation and value for the chosen hue into a small image, 64 pixels across, which is drawn stretched. It is made again only when the hue changes.
- The hue and opacity tracks are images in the same way.
- The painter does not learn gradients in M25. If later work needs them, a new decision can add them, and the picker can move to them.

### Letter keys: a text field keeps every key that types

- `TextField` keeps every key press that types a character without Ctrl, Alt or Command, even when the press carries no text, as when an input method is composing. So a letter never reaches a `CommandScope` from a text field.
- Shortcuts with no modifier, such as `B` or `Shift+B`, are allowed, and work while focus is anywhere else.

### Long press belongs to the palette

- A press held for half a second (`LONG_PRESS`, 500 ms) on a grouped tool opens its group.
- The timer is a `Task::after`, which reads `now()`, so tests move it with `advance`.
- It is not added to `Element`. If other controls need it, it can move there later.

### Today's date in core

- `dovetail_core::Date` is a day in the Gregorian calendar, with its sums in Dovetail's own code: days in a month, the day of the week, and adding days and months.
- `dovetail_core::today()` is the local date:
  - from a date set with `set_today`, for tests;
  - else from the date source the desktop crate installs, which asks the system for the local date (`GetLocalTime` on Windows, `localtime_r` on Linux and macOS);
  - else, in headless runs, the date in UTC from `SystemTime`.
- `TestUi` sets a fixed date, 2026-01-15, so tests do not change with the day they run.

### Roles

- New roles: `ColourWell`, `DateInput`, `TimeInput`, `Grid` and `GridCell`, `MenuItemCheckBox` and `MenuItemRadio`. Each maps to AccessKit's role of the same name.
- A toggle button is a `Button` that is toggled, as AccessKit and ARIA have it.
- A menu item for a toggle is a `MenuItemCheckBox`, and for a choice a `MenuItemRadio`, each toggled.

### Where the code goes

- `ToggleButton`, `ButtonGroup`, the pickers and `Calendar` go in `dovetail-controls`.
- `ToolPalette`, and the command versions of toggle buttons and button groups, go in `dovetail-commands`.
- `Hsv` and hex text for colours go in `dovetail-theme`'s colour module, beside OKLCH.

## Consequences

- Every view of a command, from menus to the palette, shows its state with no code of the application's own.
- Overflow costs one more layout pass when the toolbar's width changes. Nothing else is laid out differently.
- The colour picker works with the painter as it is. Its area is a little soft when it is drawn large, since it is a small image stretched.
- Text fields swallow letter keys with no modifier. A control that wants such keys while a text field has focus must sit inside the field, not around it.
- Tests that use dates must use `today()` and set it, not read the system clock.
- The Linux and macOS date calls, and ticks in macOS's own menu bar, can only be type-checked on Windows. They need CI and a check by hand.

## Alternatives considered

- **Overflow in layout:** layout would hide the children that do not fit and report how many. It needs a new rule in layout, and hidden boxes would still have to be left out of painting, hit tests, focus and the AccessKit tree. Measuring needs none of that, and shows no frame late because the session lays out again in the same update.
- **Gradients in the painter:** better for large areas, and the sliders could use them. But they need a new kind of box in the painter and its shader on three systems, for one control. Images are enough for now.
- **`CommandScope` skipping shortcuts while a text field has focus:** the scope sees only the key, not where focus is. Letting the field keep its keys is simpler, and also covers other places that take text.
- **Long press on `Element`:** every control would pay for it. Only the palette needs it now.
- **Command state in the application's own signals:** each view would need the signal passed to it, and menus could not show a tick by themselves.
- **Turning commands over through `History`:** view options would fill the undo list with steps the user does not think of as edits.
