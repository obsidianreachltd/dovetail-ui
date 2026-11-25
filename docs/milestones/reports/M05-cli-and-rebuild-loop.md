# M05 report — CLI and rebuild loop

| Field | Value |
|---|---|
| ID | M05 |
| Finished on | 2026-11-20 |
| Commit or tag | Not committed yet |
| Plan | [M05 plan](../plans/M05-cli-and-rebuild-loop.md) |

## Summary

The `dovetail` command has three commands: `new`, `dev` and `build`.

- `dovetail new` makes a project that builds and runs with no changes.
- `dovetail dev` rebuilds and restarts the program on every save, and keeps the window where it was. When a build fails it says so clearly, and the last working version keeps running.
- `dovetail build --release` makes an optimised program.

All five criteria are met. They were checked on Windows.

## What was delivered

- **`dovetail-cli`**, with no new dependencies ([D0002](../../decisions/D0002-build-own-stack.md)):
  - Hand-written argument parsing, with clear messages for mistakes (`main.rs`).
  - `dovetail new <name>` (`new.rs`). It checks the name and refuses a folder that is not empty. The project has `Cargo.toml`, `src/main.rs` (a small counter) and `.gitignore`. It depends on `dovetail-ui` by path, and has its own empty `[workspace]`.
  - `dovetail build [--release]` (`cargo.rs`). It reads Cargo's JSON messages to find the program, shows the compiler's own messages, and prints `Built <path>`.
  - `dovetail dev` (`dev.rs`):
    - It checks the modification times of `src/`, `assets/` and `Cargo.toml` every 200 ms.
    - It runs a copy of the program from `target/dovetail-dev/`, so the next build can replace the original on Windows.
    - It replaces the running program only after a successful build, and reports how long the restart took.
- **`dovetail-desktop`**, `dev.rs`:
  - `DOVETAIL_DEV_STATE` keeps the first window's position and size across restarts.
  - `DOVETAIL_DEV_READY` is written at the first frame, which `dovetail dev` uses to time a restart.
- **Tests:**
  - 10 unit tests: arguments, names, the template, the JSON path, unescaping compiler messages, and the file watching.
  - 2 unit tests for the window placement.
  - `tests/new_project.rs` (ignored by default, and run in CI).
- **CI:** the main job builds a new project on all three systems.

## Results

| # | Criterion | Result | Evidence |
|---|---|---|---|
| 1 | A new project builds and runs with no changes | Met | `dovetail new hello-dovetail` then `dovetail build`: built from nothing in 42 s, and the window "hello-dovetail" was shown 1.6 s after start. `new_project` test passes. |
| 2 | `dovetail dev` rebuilds and restarts on save, and keeps the window position | Met | Scripted check: window moved to (210, 140); after an edit the new program opened at (210, 140), the old one was stopped, and the same happened after a fix |
| 3 | Save-to-restart time measured and recorded | Met | 5.1–5.2 s, below |
| 4 | `dovetail build --release` makes a release program | Met | 7.3 MB `target\release\hello-dovetail.exe` in 46 s, and it shows its window in 1.5 s. `new_project` test checks it. |
| 5 | A failed build gives a clear error, and `dovetail dev` keeps running | Met | Scripted check: the compiler's error, then "the build failed; the errors are above. The last working version is still running. … fix them and save to try again." Both the old program and `dovetail dev` were still running. |

## Changes from the plan

- **Compiler messages:** these arrive inside Cargo's JSON when the program's path is wanted. `dovetail build` unescapes them and shows them as Cargo would, in colour.
- **The copied program:** this alternates between two names (`<name>-dev-0`, `<name>-dev-1`), so the next copy never overwrites the one that is running.
- **Installing the command:** for these checks, `dovetail` was installed with `cargo install --path crates/dovetail-cli` on the development machine.

## Known issues

None recorded as bugs. Limits:

- **Wayland:** programs cannot choose their own window position, so `dovetail dev` cannot keep it there. It keeps the size only.
- **Start-up time:** a restart takes about 2 s from starting the program to its first frame, mostly GPU set-up. That is the larger half of a small edit's restart time.
- **Path dependencies:** a new project depends on the Dovetail source by path, so it breaks if the source folder moves. R01 changes this to a published version.

## Measurements

These are for a debug build on Windows 11 with an NVIDIA TITAN RTX on Vulkan. The edit is one string in `src/main.rs` of the new project.

| What | Platform | Result |
|---|---|---|
| Save to restarted window (two edits) | Windows | 5.2 s and 5.1 s |
| Incremental debug build in that time | Windows | 3.0 s |
| Program start to first frame in that time | Windows | 1.9–2.1 s |
| First `dovetail build` of a new project, nothing cached | Windows | 42 s |
| First `dovetail build --release` | Windows | 46 s |
| Release program size | Windows | 7.3 MB |

## Follow-up

- Run the dev loop on Linux (X11 and Wayland) and macOS once CI or those machines are available.
- **Faster restarts:**
  - Find out why GPU set-up takes about 2 s at start.
  - Consider the platform's file-change notifications instead of the 200 ms check.
- **R01:** the template depends on published crates by version.
- **After the first release:** live UI updates without a restart.
