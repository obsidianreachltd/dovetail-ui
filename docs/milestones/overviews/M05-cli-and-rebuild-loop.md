# M05 — CLI and rebuild loop

| Field | Value |
|---|---|
| ID | M05 |
| Status | Done |
| Depends on | M04 |
| Plan | [M05 plan](../plans/M05-cli-and-rebuild-loop.md) |
| Report | [M05 report](../reports/M05-cli-and-rebuild-loop.md) |

## Summary

Build the `dovetail` command (from `dovetail-cli`) with `new`, `dev` and `build`, and a reliable rebuild-and-restart loop.

## Why it matters

This is the first thing a new developer touches. A fast development loop also speeds up all later milestones.

## What it includes

- `dovetail new <name>`, which creates a small working project.
- `dovetail dev`, which watches source files, rebuilds, and restarts the application.
- `dovetail build --release`, which makes a release build for the current platform.
- Clear messages when a build fails.

## What it leaves out

- Live updates without a restart (after the first release).
- Packaging for installers ([R01](R01-first-release.md)).
- Building for another operating system from one machine (after the first release).

## Done when

1. A project created by `dovetail new <name>` builds and runs with no changes.
2. `dovetail dev` rebuilds and restarts the application when a source file is saved, and keeps the window position.
3. The time from saving a file to the restarted application is measured and recorded.
4. `dovetail build --release` makes a release binary for the current operating system.
5. When a build fails, `dovetail dev` shows a clear error and keeps running.
