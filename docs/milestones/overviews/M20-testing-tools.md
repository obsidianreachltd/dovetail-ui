# M20 — Testing tools

| Field | Value |
|---|---|
| ID | M20 |
| Status | Not started |
| Depends on | M07, M15 |
| Plan | — |
| Report | — |

## Summary

Give application developers a supported way to test their user interface: find elements by role and label, click, type and scroll through the window itself, and check what is shown, with or without a GPU.

## Why it matters

- **What exists:** every example has a `--self-test`, built on functions such as `Context::click`, `Context::key`, `Context::ui_pixels` and the laid-out boxes. They work well, but they are internal, and each example writes its own helpers.
- **What applications need:** tests that do not break when the layout moves, so they find elements by what they are, as assistive tools do, not by position.
- **Safety:** tests that send real key presses to the system can type into the wrong window. Input sent through the window itself cannot, and needs no window in front.
- **A strength of the framework:** few desktop toolkits make testing the interface this easy. It also gives Dovetail's own examples shorter tests.

## What it includes

- **A testing package**, `dovetail-testing`, used from ordinary `cargo test` tests.
- **Running a component without a window:** the tree is built and laid out at a chosen size and scale, with no GPU, so tests are quick and run in CI everywhere.
- **Running in a real window**, for tests that need painting, the GPU or the system.
- **Finding elements** by role, label, text and state, using the same data as the AccessKit tree, with clear messages when nothing or more than one matches.
- **Input through the window:** click, double-click, drag, hover, scroll, type text, press keys and shortcuts, and paste, all without system key presses.
- **Waiting:** for tasks, timers and animations to finish, or for a condition, with a time limit.
- **Checking:** an element's text, state and place; the accessibility tree; and the painted pixels of an element compared with a saved picture, within a tolerance.
- **Time under control:** a clock that tests move forward by hand, so animations, caret blinks and delays are checked without waiting.
- **The examples' self-tests** moved onto these tools, where it makes them shorter.
- **A template test** in projects made by `dovetail new`.

## What it leaves out

- Recording tests by using the application.
- Driving other applications, or the system's own dialogs.
- Testing through the system's accessibility interfaces; `tools/access-check/` stays for that.

## Done when

1. A test can build a component without a window or GPU, find a button by its label, click it, and check the text that changes, on Windows, Linux and macOS in CI.
2. Finding an element that matches nothing or several gives a message that names what was searched for and what is there.
3. A test can type into a text field, press shortcuts and paste, with no key presses sent to the system.
4. A test moves the clock forward to finish an animation or a delay without waiting for it.
5. A painted element can be compared with a saved picture, and the difference saved when it fails.
6. A project made by `dovetail new` includes a test that passes.
