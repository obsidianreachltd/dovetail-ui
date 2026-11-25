# D0021 — Everyday controls: scrollbars, context menus and tooltips

| Field | Value |
|---|---|
| ID | D0021 |
| Status | Accepted |
| Date | 2026-11-23 |

## Context

M15 adds scrollbars, context menus and tooltips. Each could be built in more than one place: in layout and the painter, or as components; drawn by Dovetail, or by the system. Each choice changes core, layout and the desktop, so it is made once here.

## Decision

- **Scrollbars are drawn by layout and the painter**, not by a component. Layout works out a bar for each element with `Style::scroll` whose content is larger than it is, the painter draws it in the window style's colours (which the theme sets), and the window drags it. An element can turn its bars off (`Style::scrollbars`), as single-line text fields do.
- **Scrollbars float over the content's edge.** They are thin and widen under the pointer, so they take no space and layout does not change when one appears. They are always shown on Windows and Linux. On macOS they follow the system's setting, and with the usual setting they show only while scrolling.
- **Context menus are drawn by Dovetail**, as a layer built from `Commands`, like the drawn menu bar. They are not the system's own pop-up menus.
- **Tooltips are a wrapper component**, `Tooltip`, which shows a layer after the pointer rests on what it wraps, or while focus is inside it. Core gains hover and "focus inside" callbacks on elements, and a description that assistive tools read.

## Consequences

- Every scrolling element has a bar without the application doing anything, and the bar moves in the same frame as the content.
- The window, not a component, handles dragging a bar, so it is tested through the desktop's test functions.
- Context menus look the same on every system and can be checked by self-tests. They do not look exactly like the system's.
- Any element can have a tooltip, including the application's own.

## Alternatives considered

- **A `Scroll` control built from elements:** more like the other controls, but the bar would lag the content by a frame, and plain `Style::scroll` elements would have no bar.
- **Bars that take space beside the content:** layout would run twice when content first overflows, and content would get narrower.
- **The system's pop-up menus** (TrackPopupMenu, NSMenu): native look, but more platform code and hard to self-test.
- **A `tooltip` property on each control:** simpler to use, but only Dovetail's own controls would have one.
