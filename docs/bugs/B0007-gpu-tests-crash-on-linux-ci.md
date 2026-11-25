# B0007 — GPU tests sometimes crash on the Linux CI runner

| Field | Value |
|---|---|
| ID | B0007 |
| Status | Open |
| Severity | Medium |
| Milestone | M00 |
| Found on | 2026-11-25 |
| Platform | Linux |

## Summary

On GitHub's Linux runner, a GPU test binary in `dovetail-render` sometimes stops with signal 11 (SIGSEGV) before any of its tests report. It has happened to `images` and to `sharpness`. The other GPU test binaries passed in the same runs.

## Steps to reproduce

1. On the Linux CI runner (Ubuntu 24.04, lavapipe from `mesa-vulkan-drivers`, under Xvfb), run `cargo test -p dovetail-render --all-features -- --ignored`.
2. Run it again if it passes: it does not crash every time.

## Expected

Every GPU test runs and passes, as on Windows and macOS.

## Actual

```text
running 2 tests
error: test failed, to rerun pass `-p dovetail-render --test sharpness`

Caused by:
  process didn't exit successfully: `.../sharpness-... --ignored` (signal: 11, SIGSEGV: invalid memory reference)
```

## Notes

- It did not happen in 10 runs in a Docker container with the same Ubuntu, lavapipe and Rust version on a Windows machine. So it may depend on the runner's processor.
- Each test makes its own GPU device with `Gpu::headless()`, and the test harness runs tests at the same time. The suspected cause is lavapipe crashing when devices are made, or shaders compiled, on several threads at once. This is not proven: the `paint` tests also make a device each and have not crashed.
- Workaround: CI runs the Linux GPU tests with `--test-threads=1`. If they still fail, a CI step runs each GPU test binary under gdb and prints a backtrace of every thread.

## Fix
