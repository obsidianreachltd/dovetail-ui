# M25 plan — Pickers and tool palettes

| Field | Value |
|---|---|
| ID | M25 |
| Status | In progress |
| Overview | [M25 overview](../overviews/M25-pickers-and-tool-palettes.md) |
| Last updated | 2026-11-27 |

## Goal

Add the controls that tools and editors need beyond the everyday ones: date and time pickers, a colour picker, toggle buttons, button groups, and tool palettes like those in drawing programs, with commands that can be on or off.

## Starting point

- **Controls** in `dovetail-controls`, from M15 and M17:
  - `Button`, `IconButton`, `Toggle`, `CheckBox`, `RadioGroup`, `Select`, `NumberField`, `Slider`, `Tooltip`, `Tabs`, `TextField`;
  - all read `theme()`, and animate with `use_animated`.
  - `RadioGroup` and `Tabs` already move a choice with the arrow keys, with only the chosen item a Tab stop. Button groups and the palette follow them.
- **Layers:** `Select` opens its list in a `Layer::Below`, with a `Layer::Fill` behind it that closes it. The calendar, the colour picker's layer and the palette's tool groups do the same.
- **Commands** in `dovetail-commands`, from M08 and M17:
  - `CommandInfo` has an id, a name, a shortcut, `enabled` and an icon;
  - `enabled_when` gives a rule;
  - a command has no state of its own: it cannot be on or off.
  - `MenuBar`, `ContextMenu`, `Toolbar`, `CommandButton` and `CommandSearch` show commands. `MenuItem` is a command or a separator.
  - On macOS the menu bar is the system's, made in `platform/macos.rs`.
- **Accessibility:**
  - `Element::toggled`, `selected`, `expanded`, `mixed`, `range` and `text_value` exist;
  - the roles go up to M16's tree and table roles. There is no colour, date or time role, and no grid for a calendar.
- **Painting:** boxes have one colour each. There are no gradients. Images from `dovetail-image` can be drawn at any size.
- **Time:** UI code reads `dovetail_core::now()`, an `Instant`, which tests move with `advance`. Nothing gives today's date: the standard library has no local time.
- **Docking** (M10): any view can be a panel, and panels float in windows of their own.

## Steps

1. [x] **Settle the open questions**, in a decision record, [D0030](../../decisions/D0030-pickers-and-tool-palettes.md), accepted on 2026-11-27. See Prototypes and open decisions below.
2. [x] **Core:**
   - roles for a colour well, a date field, a time field, and a grid with cells for the calendar. A toggle button is a `Button` with `toggled`, as AccessKit and ARIA have it;
   - `dovetail_core::today()`, the local date, which tests can set, as they set the system's settings (M18);
   - a `Date` type, with the Gregorian calendar's sums in Dovetail's own code: days in a month, the day of the week, and adding days and months.
   - Done on 2026-11-27:
     - `Role::ColourWell`, `DateInput`, `TimeInput`, `Grid`, `GridCell`, `MenuItemCheckBox` and `MenuItemRadio`, each mapped to AccessKit's role of the same name. A menu item is named by its text, so the last three are too.
     - `crates/dovetail-core/src/date.rs`: `Date`, with `days_since_epoch` and back (Howard Hinnant's sums), `weekday`, `add_days`, `add_months` (a day past the end of the new month becomes its last day), and text as year-month-day; `Weekday`, `MONTH_NAMES`, `days_in_month` and `is_leap_year`.
     - `Time`, a time of day to the second, read and written on the 24-hour clock and read on the 12-hour one.
     - `today()`: a date set with `set_today`, else the source the desktop installs with `set_date_source`, else the date in UTC from `SystemTime`.
     - `TestUi` sets `TEST_TODAY`, 2026-01-15, a Thursday.
     - Unit tests check the sums against known dates across leap years and month ends, round trips over 1.6 million days, and text.
3. [x] **Platform:** the local date on Windows, Linux and macOS, in `crates/dovetail-desktop/src/platform/`, with a `// SAFETY:` comment on every `unsafe` block.
   - Done on 2026-11-27: `GetLocalTime` on Windows, and `time` and `localtime_r` from the C library on Linux and macOS (`platform/local_time.rs`). The desktop installs it as the date source when a loop starts, and takes it away at the end.
   - The time zone offset is not read: nothing in M25 needs it. The Linux and macOS calls are type-checked here, and need CI and a check by hand.
4. [x] **Command state:**
   - a command can be defined as on or off, or as one of a group of which one is on;
   - `set_on` and a rule like `enabled_when`, read where the command is shown, so every view follows it;
   - running a command that is on or off turns it over. Running one of a group makes it the one that is on;
   - `CommandInfo` says the state.
   - Done on 2026-11-27: `Commands::define_toggle` and `define_choice`, `set_on`, `on_when`, `is_on`, `chosen` and `group`. The first choice of a group starts on, and a command defined again keeps its state. `CommandInfo` has `on` and `group`, and `CommandInfo::missing` for an id that is not defined. A toggle's `run` is given its new state.
5. [x] **Menus show the state:** a tick or a dot in `MenuBar`, `ContextMenu` and `CommandSearch`, and in macOS's own menu bar. Assistive tools read it as checked.
   - Done on 2026-11-27: a menu that has a command with state leaves room before every item for a tick (a toggle) or a dot (a choice). The items are `MenuItemCheckBox` or `MenuItemRadio`, toggled. Command search shows the tick, and its results say "On" or "Off" in their description, since a list option that is toggled reads as selected.
   - `MenuItemSpec::Command` has `checked`, and macOS sets the item's state with `setState:`. Type-checked only.
6. [x] **`ToggleButton` and `ButtonGroup`** in `dovetail-controls`:
   - a toggle button with an icon, text, or both, pressed while on;
   - a button group across or down, with shared edges, and one choice, several, or none;
   - one Tab stop for the group, and the arrow keys, Home and End move within it;
   - command versions in `dovetail-commands`, made from a list of command ids.
   - Done on 2026-11-27 (`crates/dovetail-controls/src/group.rs`):
     - `ToggleButton`: a `Button` that is toggled, with its fill mixed in with `use_animated`;
     - `ButtonGroup` with `GroupButton`s and `Choose::One`, `Many` or `None`. The buttons sit inside one rounded track, rather than sharing drawn edges, since boxes have one radius for all corners. With one choice it is a `RadioGroup` of `RadioButton`s;
     - in `dovetail-commands`, `CommandButton` shows a toggle command as a `ToggleButton`, and `CommandGroup` makes a group from command ids: a segmented control for choices of one group, toggle buttons for toggles, and plain buttons for a mix.
     - The group's keyboard place is set when a button is pressed, not when it takes focus. Setting it on focus built the buttons again between press and release, which lost the click ([B0015](../../bugs/B0015-click-lost-when-press-rebuilds.md), fixed in the session).
7. [x] **Toolbar:**
   - toggle buttons and button groups made from commands that are on or off;
   - when it is too narrow, the buttons that do not fit go into a menu opened by a button at its end. Each can still be run from there, and its state shows.
   - Done on 2026-11-27: `Toolbar { overflow: true }` fills its width, measures its children with `on_size`, and builds only those that fit, with a More button at the end (D0030). The session lays out again in the same update, so no frame shows them all. The More button opens a layer with the rest, as they are. Without `overflow`, the toolbar is as before.
8. [x] **`ToolPalette`** in `dovetail-commands`:
   - each tool is a command, and the tools are one group, so one is chosen;
   - buttons in a given number of columns, or in rows, or as many as fit;
   - grouped tools: the button shows the last tool used from its group, with a mark in its corner. A long press, a right-click, the Menu key or Down opens the group in a layer beside it;
   - a letter key for each tool, and Shift with the letter goes through its group;
   - a tooltip with the name and the key;
   - it works in a dock panel and in a floating one, and changes between columns and rows when its panel's shape changes.
   - Done on 2026-11-27 (`crates/dovetail-commands/src/palette.rs`):
     - slots of command ids, each a choice; `columns`, or as many columns as fit the width it is given;
     - a long press (`LONG_PRESS`, 500 ms, a `Task::after`), a right-click, the Menu key or Alt+Down opens a group under its button. Down alone moves between tools, as the palette is a grid of them;
     - the palette defines a command for Shift and each group's first letter, which goes through the group;
     - one Tab stop, the arrow keys, Home and End move, and Space or Enter chooses;
     - the tooltip is left out while its group is open, since focus inside the group showed it over the menu.
9. [x] **Date and time:**
   - `DateField`: a field that reads a typed date, as year-month-day until M22, and checks it against a first and a last date;
   - a calendar that opens as a layer under the field, with the month, a week on each line, and today marked. The arrow keys move by a day and a week, Page Up and Page Down by a month, Home and End to the ends of the week, and Enter chooses;
   - the calendar on its own, for a panel;
   - `TimeField`: hours, minutes and, if asked for, seconds, each a part the arrow keys move, in a 12- or 24-hour clock.
   - Done on 2026-11-27 (`crates/dovetail-controls/src/date.rs`): `Calendar`, `DateField` and `TimeField`. Shift with Page Up or Page Down moves a year. A refused date or time says why under the field. In a time field, digits fill a part, and a number out of range is refused and starts the part again. Buttons beside it step the part with the pointer. Each day is a `GridCell` named in full, such as "Thursday 15 January 2026", and today is described as "Today".
10. [x] **Colour:**
    - a colour type with sRGB, HSV and hex, and conversion between them;
    - `ColourPicker`: a colour area, sliders for hue and opacity, and fields for hex, RGB and HSV, all kept in step;
    - the arrow keys move the point in the colour area;
    - colours the application gives, and the colours used last, kept by the application;
    - `ColourButton`: a swatch that opens the picker as a layer.
    - Done on 2026-11-27: `Hsv`, `to_hex` and `parse_hex` in `dovetail_theme::color`, beside OKLCH. `ColourPicker` and `ColourButton` in `crates/dovetail-controls/src/colour.rs`. The colour area and the slider tracks are small images, made again only when what they show changes (D0030). A grey keeps the hue it had.
11. [x] **Theme:** component themes for button groups, the palette, the calendar and the colour picker, with `..._look` functions as in M15, and the motion of pressed buttons and opening layers from `theme().motion`.
    - Done on 2026-11-27: `GroupTheme`, `CalendarTheme` and `ColourPickerTheme`, with `group_look`, `calendar_look` and `colour_picker_look`. Pressed buttons fade with `theme().motion.short()`. Layers open at once, as menus do now.
12. [x] **Examples:**
    - `examples/controls` gains the pickers, toggle buttons and button groups, in its `--self-test`;
    - a new `examples/drawing`: a canvas to draw shapes on, a tool palette in a dock panel, a toolbar of toggle commands such as Snap to grid, and a View menu with ticks. Its `--self-test` uses all of it, with `TestWindows`.
    - Add the new example to `examples/README.md` and the root README.
    - Done on 2026-11-27:
      - `examples/controls` has a Pickers tab, and twelve more self-test steps;
      - `examples/drawing` draws rectangles, ellipses, lines, pen and brush strokes with undo, a grid and snapping, a fill colour, a title field, and a dark theme from the View menu. Its self-test makes 24 checks, among them the palette floating in its own window;
      - the root README and `examples/README.md` list it. `examples/README.md` came with M19, merged into this branch on 2026-11-27;
      - Dovetail's icon set has 15 more icons, 48 in all: a calendar, a clock, a palette, drawing tools, a grid, a magnet, a hand and text alignment.
      - `TestWindows` gained `pointer_down` and `pointer_up`, to hold a press.
13. [x] **Tests**, listed under Testing below.
    - Done on 2026-11-27: `crates/dovetail-controls/tests/pickers.rs` (10 tests), `crates/dovetail-commands/tests/state.rs` (9 tests), unit tests in core, theme, controls and commands, an AccessKit tree test in `crates/dovetail-desktop/src/access.rs`, and the B0015 test in `crates/dovetail-session/tests/session.rs`.
14. [ ] **Checks by hand** on Windows, Linux and macOS, then the report.

## Prototypes and open decisions

All decided in [D0030](../../decisions/D0030-pickers-and-tool-palettes.md):

- **Toolbar overflow:** decided, the toolbar measures its children with `on_size` and builds again. The session lays out again in the same update when a reported size changes what is built, so no frame is shown late, and layout does not change.
- **The colour area:** decided, a small image drawn when the hue changes and stretched. The painter does not learn gradients in M25.
- **Letter keys for tools:** decided, a text field keeps every key that types a character without Ctrl, Alt or Command, even with no text, as while an input method composes.
- **Long press:** decided, half a second (`LONG_PRESS`), in the palette alone, with a `Task::after` that tests move with `advance`.
- **Today's date:** decided, `today()` in core: a date set by a test, else the system's local date, else the date in UTC. `TestUi` sets 2026-01-15.
- **Command state and history:** decided, turning a command over is not an edit in `History`.
- **Follow-ups, after M25:**
  - A tooltip's layer takes clicks meant for what is under it, and passes them to the element it describes. Tooltips should let the pointer through.
  - Boxes have one corner radius, so a button group cannot round only its outer corners. Corners of their own would let groups share drawn edges.
  - The drawing example's canvas draws shapes as boxes. Ellipses are rounded boxes, and lines are dots. A canvas of its own (M09) would draw them properly.
  - Months and days are in English until M22, which also brings the week's first day from the user's settings.

## Risks

- **Overflow touches layout.** A new rule in layout could change other rows. Keep it to an element that asks for it, and run every self-test.
- **Local time is platform code.** The Linux and macOS calls can only be type-checked here. They need CI, and a check by hand that today is marked on each system.
- **macOS's own menu bar** must show ticks and dots through `NSMenuItem`'s state. This can only be type-checked here, and must be run on a Mac.
- **Letter keys** that run while typing would break text fields. Test a text field next to a palette in the example.
- **Many controls at once.** Do command state, toggle buttons and button groups first, since the toolbar and the palette need them, then the palette, then the pickers.

## Testing

- **Unit tests:**
  - core: the calendar's sums, against known dates across leap years and month ends;
  - colour: sRGB, HSV and hex there and back;
  - commands: on and off, groups, rules, and the state in `CommandInfo`.
- **Headless tests with `TestUi`:**
  - every new control with the pointer and with the keyboard alone, found by role and label;
  - the palette's long press, moved with `advance`;
  - a typed date or colour out of range is refused;
  - a letter key in a text field types the letter, and does not change the tool.
- **AccessKit tree tests:** the new roles, toggle buttons' pressed state, the calendar grid, and ticks in menus.
- **Self-tests:** `examples/controls` and `examples/drawing`, in light and dark, in real windows. Run every example's self-test after the layout change for overflow.
- **By hand, on each system:**
  - the look of every control;
  - today marked in the calendar;
  - the palette in a floating panel;
  - ticks in macOS's own menu bar.

## Done when

Copied from the overview.

1. A command that is on or off shows its state in menus, the toolbar, context menus and the tool palette, and assistive tools read it.
2. Toggle buttons and button groups, with one choice, several or none, can each be used with the pointer and with the keyboard alone.
3. A toolbar too narrow for its buttons puts the rest in a menu at its end, and every one can still be run.
4. In the tool palette a tool can be chosen by a click, by its letter key, and from its group by a long press, a right-click and the keyboard. It works in a floating panel.
5. A date, a time and a colour can each be chosen with the pointer, with the keyboard alone and by typing, and a typed value out of range is refused.
6. Every new control has the right AccessKit role and state, checked by tests on the AccessKit tree.
7. The examples show every new control in the light and dark themes, and their self-tests use each one.
