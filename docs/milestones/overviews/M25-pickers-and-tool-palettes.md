# M25 — Pickers and tool palettes

| Field | Value |
|---|---|
| ID | M25 |
| Status | In progress |
| Depends on | M10, M15, M17 |
| Plan | [M25 plan](../plans/M25-pickers-and-tool-palettes.md) |
| Report | — |

## Summary

Add the controls that tools and editors need beyond the everyday ones: date and time pickers, a colour picker, toggle buttons, button groups, and tool palettes like those in drawing programs, with commands that can be on or off.

## Why it matters

- **What exists:** the everyday controls from [M15](M15-everyday-controls.md), icons from [M17](M17-icons-and-images.md), and the menus, toolbar and commands from [M08](M08-commands-and-undo.md).
- **What is missing:**
  - Forms cannot ask for a date, a time or a colour, except as text.
  - A button cannot stay pressed, and buttons cannot be joined into a group with one choice, as in a text editor's alignment buttons.
  - Editors such as the PCB editor in [R01](R01-first-release.md) need a palette of tools, with one tool chosen, grouped tools behind one button, and a letter key for each tool.
  - A command cannot be on or off, so a menu cannot show a tick, and the toolbar cannot show a pressed button.
- **Later work:** [M26](M26-rich-text.md)'s formatting toolbar is built from toggle buttons and button groups.

## What it includes

- **Commands that are on or off:**
  - a command can be on or off, or one of a group of which one is on;
  - menus show a tick or a dot, the toolbar and the palette show a pressed button, and assistive tools read the state;
  - every place a command is shown follows its state.
- **Toggle button:** a button that stays pressed until it is pressed again, with an icon, text, or both.
- **Button group:**
  - buttons joined side by side or one above the other, with shared edges;
  - with one choice (a segmented control), several choices, or none: plain buttons;
  - moved through with the arrow keys, as one stop for Tab.
- **Toolbar:**
  - toggle buttons and button groups, made from commands;
  - when it is too narrow, the buttons that do not fit go into a menu at its end.
- **Tool palette:**
  - icon buttons in one or more columns, or in rows, with one tool chosen;
  - grouped tools: a button shows the last tool used from its group, with a small mark. A long press, a right-click or the keyboard opens the rest of the group;
  - a letter key for each tool, and the same letter with Shift goes through the tools in its group, as in drawing programs;
  - a tooltip with the tool's name and key;
  - the palette can sit in a [docking](M10-docking.md) panel, float, and change between columns and rows to fit.
- **Date picker:**
  - a field that takes a typed date and checks it, with a button that opens a calendar of the month;
  - used with the pointer and with the keyboard alone: the arrow keys move by a day and a week, and Page Up and Page Down by a month;
  - a first and a last date allowed, and today marked.
- **Time picker:** a field of hours, minutes and, if asked for, seconds, each moved with the arrow keys, in a 12- or 24-hour clock.
- **Colour picker:**
  - a colour area and hue slider, an opacity slider, and fields for hex, RGB and HSV;
  - colours the application gives, and the colours used last;
  - a swatch button that opens the picker as a layer, and the picker on its own for a panel.
- **For every control:**
  - an AccessKit role, name, value and state, as M07 requires;
  - the theme's colours, text styles and shapes, with motion from `use_animated` that follows less motion;
  - a disabled state.
- **Examples:**
  - the pickers, toggle buttons and button groups added to `example-controls`;
  - a new example, a small drawing tool with a tool palette, a toolbar of toggle commands, and a menu with ticks.
  - Both have a `--self-test`.

## What it leaves out

- Dates and times in the user's own format, and month and day names in their language: [M22](M22-localisation.md). Until then the pickers show dates as year-month-day and names in English.
- The system's own colour and date dialogs. Dovetail draws its own on every system ([D0002](../../decisions/D0002-build-own-stack.md)).
- Picking a colour from anywhere on the screen. It needs a screen capture on each system.
- Time zones and calendars other than the Gregorian one.
- Rich text and its formatting toolbar: [M26](M26-rich-text.md).
- The ribbon, as in Microsoft Office.

## Done when

1. A command that is on or off shows its state in menus, the toolbar, context menus and the tool palette, and assistive tools read it.
2. Toggle buttons and button groups, with one choice, several or none, can each be used with the pointer and with the keyboard alone.
3. A toolbar too narrow for its buttons puts the rest in a menu at its end, and every one can still be run.
4. In the tool palette a tool can be chosen by a click, by its letter key, and from its group by a long press, a right-click and the keyboard. It works in a floating panel.
5. A date, a time and a colour can each be chosen with the pointer, with the keyboard alone and by typing, and a typed value out of range is refused.
6. Every new control has the right AccessKit role and state, checked by tests on the AccessKit tree.
7. The examples show every new control in the light and dark themes, and their self-tests use each one.
