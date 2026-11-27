# M15 — Everyday controls

| Field | Value |
|---|---|
| ID | M15 |
| Status | In progress |
| Depends on | M07, M08, M14 |
| Plan | [M15 plan](../plans/M15-everyday-controls.md) |
| Report | — |

## Summary

Add the controls almost every desktop application needs and Dovetail does not have yet: scrollbars, check boxes, radio buttons, drop-downs, number fields, sliders, tooltips, context menus, and a few smaller pieces.

## Why it matters

- **What exists:** buttons, switches, text fields, progress bars and dialogs, and the menu bar, toolbar and command search from M08.
- **What is missing:**
  - Areas scroll with the wheel, but show no bar, so users cannot see that there is more, or drag to it.
  - Forms and inspectors need ways to choose from a list, set a number, or turn an option on.
  - Right-clicking does nothing.
- **Later work:** the data views in [M16](M16-data-views.md) and the validation editor in [R01](R01-first-release.md) are built from these controls.

## What it includes

- **Scrollbars** on every element that scrolls (`Style::scroll`):
  - dragged with the pointer, with a click on the track moving a page;
  - shown as the system's habit is: always, or only while scrolling;
  - in the theme's colours.
- **Check box and radio buttons:** on, off, and (for check boxes) mixed, with a radio group that moves its choice with the arrow keys.
- **Drop-down (select):**
  - a list of choices that opens as a layer under the control;
  - with keyboard use: arrows, typing the first letters, Enter and Escape.
- **Number field:**
  - a text field that takes only numbers, with a range and a step;
  - up and down buttons, the arrow keys, and the mouse wheel.
- **Slider:** a value in a range, dragged or moved with the arrow keys, Page Up, Page Down, Home and End.
- **Tooltip:** short help shown after the pointer rests on an element, and while it has keyboard focus. Assistive tools read it as the element's description.
- **Context menus:** right-click, the Menu key or Shift+F10 opens a menu built from the same `Commands` as the menu bar, with the same shortcuts shown and the same items greyed out.
- **Smaller pieces:**
  - a status bar;
  - tabs outside docking;
  - a section that collapses and expands;
  - a separator;
  - a link that looks like text.
- **For every control:**
  - an AccessKit role, name, value and state, as M07 requires;
  - the theme's colours, text styles and shapes, with a component theme where a control has a look of its own;
  - a disabled state.
- **An example** with every control, in light and dark, with a `--self-test`.

## What it leaves out

- Lists, trees and tables that show many rows, and the property grid: [M16](M16-data-views.md).
- Icons in buttons and menus: [M17](M17-icons-and-images.md).
- Date and time pickers, and a colour picker: [M25](M25-pickers-and-tool-palettes.md).
- Toast notifications: after R01.

## Done when

1. Every scrolling element shows a scrollbar that can be dragged, and a click on its track moves one page.
2. Check boxes, radio groups, drop-downs, number fields and sliders can each be used with the pointer and with the keyboard alone.
3. A tooltip shows on hover and on focus, and assistive tools read it.
4. A right-click opens a context menu of commands, with their shortcuts, and greyed-out items cannot be chosen.
5. Every new control has the right AccessKit role and state, checked by tests on the AccessKit tree.
6. The example shows every control in the light and dark themes, and its self-test uses each one.
