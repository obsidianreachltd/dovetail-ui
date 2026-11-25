# M05 plan — CLI and rebuild loop

| Field | Value |
|---|---|
| ID | M05 |
| Status | Done |
| Overview | [M05 overview](../overviews/M05-cli-and-rebuild-loop.md) |
| Last updated | 2026-11-20 |

## Goal

Build the `dovetail` command (from `dovetail-cli`) with `new`, `dev` and `build`, and a reliable rebuild-and-restart loop.

## Starting point

- `dovetail-cli` builds a `dovetail` binary that prints its version and usage only.
- M04 gives `dovetail::run(title, root)`, controls and `examples/counter`.
- The Dovetail crates are not published yet (R01). A new project must therefore depend on them by path.
- [D0002](../../decisions/D0002-build-own-stack.md) keeps dependencies small. The CLI parses its own arguments and watches files by checking their modification times, with no new dependencies.

## Steps

1. [x] **Window position across restarts.** When `DOVETAIL_DEV_STATE` names a file, `dovetail-desktop` reads the first window's position and size from it at start, and writes them whenever the window moves or is resized.
2. [x] **Ready signal.** When `DOVETAIL_DEV_READY` names a file, `dovetail-desktop` writes it after the first frame is shown. `dovetail dev` uses it to measure the time from saving to the restarted application being on screen.
3. [x] **Arguments.** Parse `new`, `dev`, `build`, `--release`, `--help` and `--version` by hand. An unknown command or option gives a clear message and the usage.
4. [x] **`dovetail new <name>`.**
   - It checks the name (a valid crate name) and refuses a folder that exists and is not empty.
   - It writes `Cargo.toml`, `src/main.rs` (a small counter application) and `.gitignore`.
   - The project depends on `dovetail-ui` by path, at the Dovetail source the CLI was built from, until the crates are published.
   - The `Cargo.toml` has an empty `[workspace]`, so a project made inside another workspace still builds alone.
5. [x] **`dovetail build [--release]`.**
   - It runs `cargo build` with JSON messages, so that it can report the path of the built program.
   - Cargo's own error messages are shown unchanged.
6. [x] **`dovetail dev`.**
   - It builds, copies the program to `target/dovetail-dev/`, and runs the copy. On Windows the linker cannot replace a program that is running, so the copy lets the next build succeed while the old copy is still open.
   - It watches `src/`, `Cargo.toml` and `assets/` by checking modification times every 200 ms.
   - When something changes, it rebuilds. If the build succeeds, it stops the old program and starts the new one. If the build fails, it prints a clear message after Cargo's errors, leaves the old program running, and waits for the next save.
7. [x] **Tests:**
   - unit tests for argument parsing, name checks and the JSON path extraction;
   - an integration test (ignored by default, because it builds a whole project) that runs `dovetail new` and builds the result;
   - a scripted check of the dev loop on Windows.
8. [x] **Measure** the time from saving a file to the restarted window being on screen, for a small edit to the new project. Record it. On 2026-11-20 on Windows it was 5.1–5.2 s: 3.0 s of incremental build and about 2.0 s from starting the program to its first frame. See the report.

## Prototypes and open decisions

- **Path dependencies:** a new project depends on the Dovetail source by path. R01, when the crates are published, changes the template to a version.
- **Watching files:** checking modification times every 200 ms is simple and works the same on every system. The platform's change notifications would be quicker, but need a library or code for each system. Revisit if the measurement shows the 200 ms check matters.
- **Restoring state** beyond the window position, such as component state, is part of live updates, which come after the first release.

## Risks

- **Building a new project from nothing is slow,** because wgpu and its dependencies compile. The integration test is ignored by default, and shares a target folder between runs.
- **Stopping the old program** is a hard kill, so the program gets no chance to clean up. That is acceptable for development. Saving the window position on every move means nothing is lost.
- **Editors** save in different ways (write in place, or write a new file and rename it). Checking modification times sees both.

## Testing

- `cargo test -p dovetail-cli` runs the unit tests.
- `cargo test -p dovetail-cli -- --ignored` creates and builds a project.
- The scripted dev-loop check covers:
  - start `dovetail dev`;
  - move the window;
  - edit `src/main.rs`, and check that the program restarts and the window comes back in the same place;
  - make a build error, and check the message and that the old program still runs;
  - fix it, and check that the program restarts.

## Done when

Copied from the overview.

1. A project created by `dovetail new <name>` builds and runs with no changes.
2. `dovetail dev` rebuilds and restarts the application when a source file is saved, and keeps the window position.
3. The time from saving a file to the restarted application is measured and recorded.
4. `dovetail build --release` makes a release binary for the current operating system.
5. When a build fails, `dovetail dev` shows a clear error and keeps running.
