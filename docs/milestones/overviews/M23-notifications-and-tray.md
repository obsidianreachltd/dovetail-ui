# M23 — Notifications and the tray

| Field | Value |
|---|---|
| ID | M23 |
| Status | Not started |
| Depends on | R01 |
| Plan | — |
| Report | — |

## Summary

Let applications reach the user outside their windows: system notifications, an icon in the system tray or menu bar, and a badge or progress on the taskbar or dock icon.

## Why it matters

- **What applications need:** a long task that finishes while its window is hidden should say so. Tools that run in the background need a tray icon with a menu.
- **Progress outside the window:** Windows' taskbar and macOS's dock can show the progress of [M11](M11-documents.md)'s tasks and a count of things to look at.
- **Why after the first release:** none of the ten acceptance points in [R01](R01-first-release.md) needs this, and each system does it differently.

## What it includes

- **Notifications:** a title, text and icon, with a click that brings the application to the front and runs a command, and up to two buttons where the system allows it.
- **A tray icon:** on Windows' notification area, macOS's menu bar and Linux's status notifier, with a tooltip, a menu made with `dovetail-commands`, and a click.
- **The taskbar and dock icon:** progress, a badge with a count, and asking for attention.
- **Running in the background:** an application that keeps running with its windows closed, from its tray icon.
- **An example** with a `--self-test` for what can be checked from inside the program.

## What it leaves out

- Notifications sent from a server while the application is not running.
- Widgets on the desktop, and the system's own widget areas.

## Done when

1. A notification shows with its title, text and icon, and clicking it brings the application to the front and runs its command, on Windows, Linux and macOS.
2. A tray icon shows its menu, and its commands run.
3. A task's progress shows on the taskbar or dock icon, and a badge shows a count.
4. An application can keep running from its tray icon with no window open.
5. The example's self-test checks what it can, and the plan lists the manual checks for the rest.
