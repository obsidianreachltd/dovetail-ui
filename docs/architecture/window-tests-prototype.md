# Real-window tests: prototype report

This report answers step 2 of the [M20 plan](../milestones/plans/M20-testing-tools.md): can a test written as straight-line code drive real windows from `cargo test`, on Windows, Linux and macOS? The answer decides how M20's real-window tests run, which [D0026](../decisions/D0026-real-window-tests.md) records.

The code is in `prototypes/window-tests`. It is throw-away code, but it stays in the repository so these results can be checked again. Run it with `cargo test -p prototype-window-tests --test pumped -- --ignored`.

## The question

- **winit allows one event loop for each process.** Making a second one fails.
- **On macOS, the event loop and windows must be on the main thread.** libtest, the harness behind `#[test]`, runs each test on a thread of its own. So a `#[test]` cannot open a window on macOS.
- **The examples' self-tests** hand the loop to winit with `run_app`, and react to callbacks with a machine of steps. A test for an application should read as a list of actions and checks instead.

## The design tried

- **A test target with `harness = false`.** Cargo runs its `main` on the main thread, and `cargo test` still builds and runs it. `main` runs each test in turn, catches a failing test's panic so the others still run, and prints results as libtest does. It runs only with `--ignored`, so `cargo test --workspace` doesn't open windows.
- **One `Driver` owns the event loop** for all the tests in the target.
- **Waiting pumps the loop.** `Driver::wait_until(what, limit, condition)` runs the event loop one step at a time with winit's `pump_app_events`, at most 5 ms each, until the condition is true or the time limit passes. The test has control between steps.
- **Commands are queued.** Opening, resizing and closing a window need winit's `ActiveEventLoop`, which exists only inside a callback. The test queues a command, and the next pump carries it out.
- **Windows draw with Dovetail's** `Gpu` and `WindowSurface`, as the desktop does.

## Results

Six tests ran in one process, on one event loop:

| Test | What it checks |
|---|---|
| `a_window_opens_and_draws` | A window opens and a frame is shown |
| `a_resized_window_draws_at_its_new_size` | After a resize, the new physical size is reported and drawn |
| `two_windows_draw_at_once` | Two windows draw, and closing one leaves the other |
| `ten_tests_in_turn_share_the_loop` | Ten windows opened and closed one after another, with no window left behind |
| `a_wait_that_never_ends_fails_with_a_message` | A wait gives up after its time limit, and names what it waited for |
| `a_second_event_loop_is_refused` | winit refuses a second event loop: "EventLoop can't be recreated" |

| System | Result | Notes |
|---|---|---|
| Windows 11, NVIDIA TITAN RTX, Vulkan | All 6 pass, 2026-11-25 | Runs on the main thread |
| Linux, CI (Xvfb, lavapipe) | All 6 pass, 2026-11-25 | X11 under Xvfb |
| macOS, CI (Apple Paravirtual device, Metal), first run | 3 of 6 pass, 2026-11-25 | Runs on the main thread, and the first window draws in 785 ms. Every window opened after the last one closed never drew. |
| macOS 26.7 (`macos-26-arm64`), CI, second run, as it is | All 6 pass, 2026-11-25 | First frame after 948 ms; 83 ms for each window after that |
| macOS 26.7, CI, second run, bringing windows to the front | All 6 pass, 2026-11-25 | First frame after 615 ms; 94 ms for each window after that |
| macOS, CI, third run (the check before merging) | All 6 pass both ways, 2026-11-25 | No warning from the run without bringing windows to the front |

### Windows that never drew on macOS

On the first macOS run, the first test passed. Every test after it that opened a window timed out waiting for its first frame, and the tests that opened no window passed.

- **The difference is the last window closing.** Each test closes its windows before the next one starts. M01's window self-test, which passed on macOS, opens its second window while the first is still open.
- **The cause is not known.** The first guess was that, once its last window closes, the program stops being the active one, and later windows are not drawn. The second run disproves that as the whole story: without bringing windows to the front, all six tests passed. So the failure happens only sometimes.
- **A bug in the prototype made the output worse.** A failed test left its windows open. The next test then counted them as its own, and the summary said "-2 passed". This is fixed: a test that leaves windows open fails, and the runner closes them before the next test.
- **What the prototype records now**, for each window still open when a test fails: the `RedrawRequested` events it got, frames shown and skipped, the last `Occluded` and `Focused` events, and whether it is visible. The second run did not fail, so there is no record of a failure yet.
- **Bringing windows to the front** (`PROTOTYPE_FRONT=1`, `focus_window`) also passed. It is not shown to fix anything, but it does no harm, and it is the usual state for a window a test drives.
- **CI keeps running both**, so a later failure shows whether it happens without bringing windows to the front, with it, or both.

## Measurements

On Windows, in a debug build:

| What | Result |
|---|---|
| All six tests | 2.9 s |
| The first window to its first frame, including setting up the GPU | 1 s |
| Opening a window, drawing a frame and closing it, once the GPU is set up | 105 ms each |
| A resize to its new size drawn | 13 ms |

## What decides

- **macOS passes,** apart from one run that failed for an unknown reason. So real-window tests can be ordinary `cargo test` targets. Each application adds one `harness = false` target for its window tests. Headless tests stay ordinary `#[test]` functions.
- **The cost:** a `harness = false` target has none of libtest's options, such as running one test by name. M20's runner must add the ones that matter: a name filter, `--ignored`, and a summary.
- **One loop for all tests:** the tests in a target share the GPU device, so only the first pays to set it up. A test that leaves a window open must fail, as the prototype checks.
- **The desktop's `Runner`** is already an `ApplicationHandler`, so the same pumping can drive it in place of `run_app`. Self-test hooks such as `Context::click` need an `ActiveEventLoop` too, so they would be queued in the same way.

## Recommendation

It works on all three systems. One macOS run failed, and its cause is not known. M20's real-window tests use a `harness = false` target and a driver that pumps the event loop. [D0026](../decisions/D0026-real-window-tests.md) records this. Until the macOS failure is understood, the runner brings each window to the front as a precaution, and says what each window received when a wait fails. This was built in M20 step 8 (`dovetail_testing::windows`), and CI runs its tests in place of the prototype.
