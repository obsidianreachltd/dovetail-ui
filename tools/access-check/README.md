# Accessibility checks on Windows

These PowerShell scripts read Dovetail windows through UI Automation, the interface Narrator uses (M07). Build the examples first with `cargo build -p example-counter -p example-text-field -p example-form -p example-commands`, then run from the workspace root, for example:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools\access-check\check-counter.ps1 -Exe target\debug\counter.exe
```

| Script | Example | What it checks |
|---|---|---|
| `check-counter.ps1` | `counter.exe` | The tree, pressing the button through UI Automation, and moving focus to it |
| `check-text-fields.ps1` | `text-field.exe` | Named text fields, focus, setting a value, reading the text and caret, and selecting a word |
| `check-switch.ps1` | `form.exe` | The form's tree, and flipping the switch with the toggle pattern |
| `check-focus.ps1` | `text-field.exe` | That focus moved by Tab and Shift+Tab shows in UI Automation |
| `check-menus.ps1` | `commands.exe` | The menu bar, menus and toolbar; disabled commands disabled in both; choosing items; and command search opening from a menu. With `-Shots <folder>`, saves pictures of an open menu and of command search |

Only `check-focus.ps1` sends key presses. Before each one it checks that the example's window is in front, and stops without sending anything if it is not. The others work only through the window's UI Automation interface.

They find the window by process and title, not by `MainWindowHandle`: windows start hidden for AccessKit, so Windows may report winit's hidden helper window as the main one.
