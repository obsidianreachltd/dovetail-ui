# D0026 — Real-window tests run from `cargo test`, driving the event loop step by step

| Field | Value |
|---|---|
| ID | D0026 |
| Status | Accepted |
| Date | 2026-11-25 |

## Context

M20 gives applications tests that drive real windows, for what needs painting, the GPU or the system. Three things limit how these tests can run:

- **winit allows one event loop for each process.** A second one is refused: "EventLoop can't be recreated".
- **On macOS, the event loop and windows must be on the main thread.** libtest, behind `#[test]`, runs each test on a thread of its own.
- **`run_app` keeps the loop until the program ends.** The examples' self-tests react to callbacks with a machine of steps, from 130 to 770 lines each. Tests for an application should read as a list of actions and checks.

The [prototype](../architecture/window-tests-prototype.md) tried a design on all three systems. It passed on each, apart from one macOS run that failed for a reason not yet known.

## Decision

Real-window tests are a test target with `harness = false`. Its `main` runs on the main thread and drives the one event loop a step at a time with winit's `pump_app_events`.

- **One driver for each test target** owns the event loop and the GPU. Its tests run one after another and share them.
- **Waiting pumps the loop.** A test waits for a condition with a time limit, and the message names what it waited for.
- **Actions are queued.** Opening a window and sending input need winit's `ActiveEventLoop`, which exists only inside a callback. The test queues them, and the next step carries them out.
- **New windows are brought to the front and given focus**, as a precaution. One macOS run of the prototype failed: windows opened after the last one closed were never drawn. The next run passed with and without bringing windows to the front, so the cause is not known, and this is not shown to fix it.
- **A wait that fails says what each window received:** redraw requests, frames shown and skipped, and its last hidden and focus events. A failure that comes back can then be traced.
- **The runner is part of `dovetail-testing`.** It runs each test in turn and catches a failing test's panic, so the other tests still run. It prints results as libtest does. It supports `--ignored` and a filter by name.
- **Every test closes its windows.** A test that leaves one open fails, and the runner closes it before the next test.
- **The desktop's `Runner` is driven this way**, in place of `run_app`, so tests run the framework's own window code.
- **Headless tests**, with no window and no GPU, stay ordinary `#[test]` functions.

## Consequences

- An application's window tests run with `cargo test`, on all three systems.
- A test reads as straight-line code: open, act, wait, check.
- Tests share the GPU set-up. On Windows, the first window took 1 s to its first frame. After that, opening a window, drawing and closing it took about 105 ms.
- A `harness = false` target has none of libtest's other options. The runner must add any that matter later.
- Window tests take focus on a developer's machine while they run, as the self-tests do. Input still goes through the window, so nothing is typed into other programs.
- A test target runs its window tests one at a time. Splitting tests across targets runs them in separate processes.

## Alternatives considered

- **Ordinary `#[test]` functions with an event loop on any thread.** winit allows this on Windows and X11, but not on macOS, and still only one loop for each process.
- **A program for each test, as the self-tests are.** It works everywhere, but each test pays the GPU set-up, and it is not part of `cargo test`. It remains the fallback if pumping stops working on a system.
- **`run_app` with tests as callbacks.** This is how the self-tests work today. The tests are long machines of steps, and are hard to read and write.
