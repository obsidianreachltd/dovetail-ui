# M07 plan — Accessibility

| Field | Value |
|---|---|
| ID | M07 |
| Status | In progress |
| Overview | [M07 overview](../overviews/M07-accessibility.md) |
| Last updated | 2026-11-22 |

## Goal

Build an AccessKit accessibility tree from the component tree, keep it in step with changes and focus, and test it with screen readers on each OS ([D0003](../../decisions/D0003-accesskit-for-accessibility.md)).

## Starting point

- **M06** is finished except its manual input-method checks (criterion 3). M07 does not depend on them. The following already exist:
  - text fields with a caret, selection and an input method;
  - keyboard focus with Tab, a focus ring, and keys that go to the focused element;
  - pointer handling, and scrolling.
- **Elements** (`dovetail_core::Element`) already say what they do: `focusable`, `on_press`, `on_key`, `on_pointer`, `text_cursor`. Text is in text children. Nothing yet says what an element *is* (a button, a switch, a text field) or what it is called.
- **Changes** already mark accessibility as out of date: `Tree::update` reports an `Invalidation` with an `accessibility` flag when focus-related parts of an element change (M02, M06).
- **Layout** gives every box a stable key (`BoxKey`) between updates, and its place in window coordinates.
- **AccessKit** connects through `accesskit_winit` `=0.34.2` and `accesskit` `=0.25.2` ([D0015](../../decisions/D0015-accesskit-winit-adapter.md)). It works with our winit 0.30.13.
- **Testing:** Windows has UI Automation, which Narrator uses. It can be read and driven from PowerShell, so the tree, focus and actions can be checked there without listening to speech. VoiceOver and Orca need macOS and Linux machines.

## Steps

1. [x] Settle how AccessKit is connected: [D0015](../../decisions/D0015-accesskit-winit-adapter.md), on 2026-11-22.
2. [x] **Describing elements**, done on 2026-11-22. `dovetail-core` has `Role` (button, switch, check box, text input, multi-line text input, label, heading, group, image), and `Element` has `role`, `label` (the accessible name), `toggled`, and `on_access` for text edits from tools (`AccessAction`). Changes to them mark accessibility as out of date. Core still has no dependencies.
3. [x] **Building the tree**, done on 2026-11-22 (`dovetail-desktop/src/access.rs`):
   - The window is the root, named by its title, with the display scale as its transform.
   - Elements with a role, or that take focus or can be pressed, become nodes. Other elements pass their children up. Text becomes a label, and blank text is left out.
   - Buttons, switches and check boxes are named by their text, which is not read again.
   - Text fields have their value, a text run for each line (grapheme lengths, positions, widths and word starts), and the selection.
   - Node ids are hashes of the boxes' keys, so they stay the same between updates.
4. [x] **Updating only what changed**, done on 2026-11-22. The nodes last sent are kept, and each update sends only those that differ. Nothing is built while no tool listens (`update_if_active`). When a tool first asks, the whole tree is sent.
5. [x] **Connecting each UI window**, done on 2026-11-22:
   - The event loop carries `accesskit_winit`'s events as user events.
   - A UI window is made hidden, given its adapter, then shown.
   - Window events go to the adapter.
6. [x] **Focus**, done on 2026-11-22: every update gives the focused element's node, or the window's.
7. [x] **Actions**, done on 2026-11-22. Focus moves keyboard focus and shows the ring. Click presses. Setting the selection, replacing the selected text and setting the value go to the element's `on_access`.
8. [x] **Built-in controls**, done on 2026-11-22:
   - `Button` is a button.
   - `Toggle` is a switch, with its state.
   - `TextField` is a text input, or a multi-line one. It carries out tools' edits as undoable edits, and has a new `label` property for its name.
   - `Text` becomes a label, and containers pass their children up.
   - The examples name every field.
9. [x] **Tests**, done on 2026-11-22:
   - Four unit tests in `access.rs` cover roles, names, states and actions; blank text; sending only changes; focus; text runs and selections with a combining accent; and turning tools' requests into presses, focus and edits.
   - On Windows, the scripts in `tools/access-check/` read the examples through UI Automation:
     - **The counter:** "Window 'Counter' > Text 'Count: 0', Button 'Increment'". Pressing twice through UI Automation shows "Count: 2", and focus moves to the button.
     - **The text fields:** named Edit controls. Setting a value updates the greeting, and the text pattern reads the text and caret and selects a word.
     - **The form's switch:** flips from Off to On through the toggle pattern.
     - **Focus:** follows real Tab and Shift+Tab, each key sent only after checking the window is in front.
10. [ ] **Manual checks** with Narrator, VoiceOver and Orca, with the checklist below.

## Prototypes and open decisions

- **How to connect AccessKit:** decided, [D0015](../../decisions/D0015-accesskit-winit-adapter.md).
- **How much of a text field to describe:** the full text, as text runs with the selection. With less, screen readers read the value but cannot follow the caret, and criterion 4 asks for a text field read correctly.
- **Which boxes become nodes:** elements with a role, text, and plain containers that hold them. Containers are kept, because AccessKit needs every node's parent, and they are cheap.

## Risks

- **The Linux build** pulls in many crates for the accessibility bus ([D0015](../../decisions/D0015-accesskit-winit-adapter.md)). CI must build it.
- **Screen readers differ.** They are checked by hand on each system, with the same checklist.
- **Showing windows later:** windows must be created hidden for AccessKit. The window example's self-test and `dovetail dev`'s window placement must still work. Checked: all four self-tests pass. A side effect: Windows may now report winit's hidden helper window as a process's main window, so tools that find windows by `MainWindowHandle` must find them by title instead.
- **Cost:** building the tree for every frame would be slow. It is built only while a tool listens, and only changes are sent.

## Testing

- **Unit tests** in `dovetail-desktop`, without a window:
  - the tree from a laid-out view, with roles, names, states, bounds, focus and text runs;
  - a second update that sends only what changed;
  - actions turned into presses, focus and text edits.
- **Windows, automatic:** a PowerShell script reads the window through UI Automation, as Narrator does. With `examples/counter` and `examples/text-field`, it:
  - finds the button and the text by name and type;
  - checks that focus follows Tab;
  - presses the button through UI Automation and checks that the count changes;
  - reads and changes a text field's value.

  It sends no key presses to other windows: it works only through the window's own UI Automation interface.
- **Manual, on each system**, with Narrator (Windows), VoiceOver (macOS) and Orca (Linux), using `examples/counter` and `examples/text-field`:
  1. The window's title is read when it opens.
  2. Tab to the button: "Increment, button" is read.
  3. Press it with the screen reader's own command (Narrator: Caps Lock+Enter; VoiceOver: VO+Space; Orca: Enter on the button). The count changes, and the new count can be read.
  4. In the text field example, Tab to the name field: its name or value and "edit" or "text field" are read.
  5. Type a word, then move the caret with the arrows: each letter is read as the caret passes it.
  6. Select text with Shift and the arrows: the selection is read.
  7. The toggle in the counter or form reads as a switch, with on or off, and changes when pressed.

## Done when

Copied from the overview.

1. Every built-in control declares a role, name, state and actions.
2. The AccessKit tree is built from the component tree and updated only where it changed.
3. Focus in the accessibility tree always matches keyboard focus.
4. Narrator (Windows), VoiceOver (macOS) and Orca (Linux) read the Counter example and a text field correctly.
5. Actions from assistive tools, such as pressing a button, work.
