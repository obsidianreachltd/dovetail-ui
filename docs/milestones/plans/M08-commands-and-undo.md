# M08 plan — Commands and undo

| Field | Value |
|---|---|
| ID | M08 |
| Status | In progress |
| Overview | [M08 overview](../overviews/M08-commands-and-undo.md) |
| Last updated | 2026-11-22 |

## Goal

Build the shared command system in `dovetail-commands`. It covers menus, toolbar buttons, shortcuts and command search, plus undo and redo.

## Starting point

- **M06** gives keyboard focus, keys that go to the focused element and then its parents, and text fields with their own undo. Its manual input-method checks are still open; M08 does not depend on them.
- **M07** gives the accessibility tree. New controls must declare their roles, names and states there.
- **Missing pieces the commands need:**
  - **Layers.** Menus and command search must appear above the window's content. Today every element sits inside its parent's box, and is drawn and hit in tree order.
  - **Taking focus.** Command search must put the caret in its search box when it opens, and give focus back when it closes. Today focus moves only by click or Tab.
  - **Keys with nothing focused.** Shortcuts must work when no element has focus. Today such keys reach no element.
  - **Disabled controls.** `Button` cannot be disabled yet.
- **Native menus:** winit has none. The macOS menu bar is built with our own code in `dovetail-desktop/src/platform/macos.rs`, through the Objective-C runtime, as D0013 allows for platform features. It can be type-checked here, but only run on a Mac.
- **The crate:** `dovetail-commands` is in the [crate map](../../architecture/crates.md) and does not exist yet. This plan is the reason the rules ask for.

## Steps

1. [x] Settle the open questions, decided on 2026-11-22:
   - macOS menus use our own code, not a menu library;
   - menus and command search appear as layers inside the window, not as separate windows.
2. [x] **Layers**, done on 2026-11-22:
   - `Style::layer` (`Layer::Fill`, `Below`, `TopCentre`, `At`) and `Style::modal` are in `dovetail-core`.
   - Layout lifts a layer out of its parent's box, sizes it against the window, places it there (moving it to fit inside), and draws it after everything else. Nested layers go on top of their parents. A layer's parent stays the element it belongs to, so keys reach that element.
   - Hit testing now finds the topmost box under the pointer, then the nearest element above it that handles the event. This is the same for presses, focus by click, pointer handlers and the mouse wheel.
   - While a modal layer shows, Tab stays inside the topmost one.
   - Layers are in the accessibility tree, under the window.
   - Tested in `dovetail-layout/tests/layers.rs` (5 tests).
3. [x] **Focus and keys**, done on 2026-11-22:
   - `Element::autofocus` makes an element take focus when it appears.
   - Windows keep a short history of focus, and give it back when the focused element goes away, as when a menu or command search closes.
   - With nothing focused, keys go to elements that handle keys but cannot take focus, topmost first.
   - `NamedKey` gained F1 to F12 and Insert.
4. [x] **Disabled controls**, done on 2026-11-22:
   - `Element::disabled` is in the accessibility tree.
   - `Button` has `enabled`. A disabled button is grey, and cannot take focus or be pressed.
   - Single-line text fields now leave Up and Down to their parents, which command search uses for its list.
   - `TextField` has `autofocus`.
5. [x] **Commands**, done on 2026-11-22. `dovetail-commands` has `Commands`, a copyable handle made with `use_state(Commands::new)`, with:
   - `define`, `set_enabled`, `enabled_when` (a rule checked when the command is shown or run), `is_enabled`, `run`, `info` and `list`;
   - `search`, which matches the query's letters in order and favours the starts of words.
6. [x] **Shortcuts**, done on 2026-11-22:
   - `Shortcut` is read from text such as `Mod+Shift+Z`, matches only presses with exactly its modifiers, and is shown as "Ctrl+Shift+Z" or "⇧⌘Z".
   - `CommandScope` runs commands from their shortcuts, after a focused text field has had its own keys.
   - `set_shortcut` refuses a shortcut that another command has and returns the clash. `reset_shortcut` gives a command back its own. `conflicts` lists clashes between the shortcuts commands were defined with.
7. [x] **Undo and redo**, done on 2026-11-22: `History` with `apply`, `undo`, `redo` and the names of both, and `add_commands`. That defines Undo (Mod+Z) and Redo (Mod+Y, or Mod+Shift+Z on macOS), each enabled only when there is something to undo or redo.
8. [x] **Where commands appear**, done on 2026-11-22:
   - **`CommandButton` and `Toolbar`.**
   - **`MenuBar`:** a menu opens below its title, as a layer. F10 opens the first menu, the arrow keys move, Enter chooses, and Escape or a click outside closes it. Items show their shortcuts, and disabled items are grey and cannot be chosen.
   - **`CommandSearch`:** Mod+Shift+P opens a modal box at the top centre over a dimmed window, with the caret in it. Typing narrows the list, Up and Down choose, Enter runs, and Escape or a click outside closes it.
   - **Accessibility:** they have menu, menu item, list box and toolbar roles.
9. [x] **Native menus on macOS**, written on 2026-11-22 and type-checked:
   - `dovetail-core` has `MenuModel` and the `NativeMenus` host. The desktop crate installs `MacMenus` from `platform/macos.rs` on macOS, and `MenuBar` then hands it its menus instead of drawing them.
   - It builds `NSMenu`s through the Objective-C runtime, with an application menu that has Quit.
   - Shortcuts become key equivalents, including function and arrow keys.
   - A class made at run time (`DovetailMenuTarget`) turns a chosen item's tag back into its command.
   - **Not yet run on a Mac.**
10. [x] **`examples/commands`**, done on 2026-11-22:
    - **The editor:** a list editor with Add item, Remove last item, Clear the list, Lock, Undo and Redo, all undoable where they edit. It has menus, a toolbar, a lock switch, command search, and a field that sets Add item's shortcut and reports conflicts.
    - **Its `--self-test`:**
      - runs Add item from a shortcut, the toolbar, the menu and command search;
      - undoes twice and redoes once;
      - checks that locking disables the button and the shortcut;
      - checks that giving Add item Ctrl+Z is refused with the reason, and that Ctrl+J then works.
11. [ ] **Tests and checks:**
    - **Done:** unit tests for shortcuts, commands and history (8); tests of the controls together through layout and key dispatch (2), with one command run from all four places and disabled in all of them; and the example's self-test.
    - **UI Automation on Windows** (`tools/access-check/check-menus.ps1`):
      - It reads the menu bar, the menus and the toolbar.
      - With an empty list, "Remove last item" is disabled in both the menu and the toolbar.
      - Choosing an item through UI Automation runs it.
      - Command search opens from the Edit menu.
    - **Still to do:** the manual check on macOS.

## Prototypes and open decisions

- **macOS menus:** decided, our own code in the platform module (D0013).
- **Pop-ups:** decided, layers inside the window. They cannot extend past the window's edges, so they move to fit inside it.
- **Shortcuts in text fields:** a focused text field handles its own Ctrl+Z, Ctrl+C and so on before the application's commands see the key. This is what users expect: Undo in a text field undoes the typing.

## Risks

- **macOS menus cannot be run here.** The code is type-checked for macOS, and checked by hand on a Mac before M08 is done.
- **On macOS, menu shortcuts are handled by the system before the window sees the key.** So a menu's Command+Z would undo the document even in a text field. This is not handled yet: it is a follow-up for the Mac check, where it can be seen. Elsewhere a focused text field gets its keys first.
- **Changing hit testing** to "topmost box first" could change which element a click reaches. The existing tests and self-tests must all still pass.

## Testing

- **Unit tests:**
  - layers: placement, drawing order, hit testing and Tab staying inside modal layers;
  - focus taken on appearing, and given back afterwards;
  - commands: running, enabling and disabling, and rules that follow state;
  - shortcuts: parsing, display, matching, changing and conflicts;
  - history: undo, redo, and the commands' enabled state;
  - menus and command search, through the tree, layout and key dispatch, with no window.
- **`examples/commands --self-test`** runs one command from a shortcut, a toolbar button, a menu and command search, disables it and checks every place it appears, undoes and redoes, and changes a shortcut and reports a conflict.
- **Windows:** a UI Automation check reads the menu bar, the menus and the toolbar, and sees disabled items as disabled.
- **macOS, by hand:**
  1. The menu bar shows the application's menus, with their shortcuts.
  2. Choosing a menu item runs its command.
  3. A disabled command is greyed out in the menu.
  4. The shortcuts shown in the menu work.
  5. Quit in the application menu closes the application.

## Done when

Copied from the overview.

1. A command that is defined once can be run from a menu item, a toolbar button, a shortcut and command search.
2. Enabling or disabling a command changes every place it appears.
3. Native menus work on macOS, and in-window menus work on Windows and Linux.
4. Tests show that edits made through commands can be undone and redone.
5. Shortcuts can be changed, and conflicts are reported.
