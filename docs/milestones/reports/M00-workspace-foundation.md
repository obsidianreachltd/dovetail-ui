# M00 report — Workspace foundation

| Field | Value |
|---|---|
| ID | M00 |
| Finished on | 2026-11-25 |
| Commit or tag | `13fd236`, merged into `main` from the `CI_Fixes` branch |
| Plan | [M00 plan](../plans/M00-workspace-foundation.md) |

## Summary

The repository is a Cargo workspace, its licence is chosen, and CI builds, tests and lints it on Windows, Linux and macOS. All six criteria are met. The last one, CI passing on all three systems, was met on 2026-11-25, when the `CI_Fixes` branch passed and was merged into `main`.

## What was delivered

- **The workspace:** a virtual workspace at the root, with shared version, edition 2024, licence, lints and dependency settings. It started with the seven framework crates and `dovetail-cli`, and has 15 crates now.
- **Shared settings:** `rustfmt.toml`, `[workspace.lints]` and `.gitignore`.
- **The toolchain pin:** `rust-toolchain.toml` pins Rust 1.100.0 for building, linting and testing, and CI uses the same version. The oldest supported version stays `rust-version` 1.89.
- **Line endings:** `.gitattributes` keeps text files at Unix line endings on every system, as `rustfmt.toml` asks, and marks the image test data as binary.
- **The publish guard:** `publish = false` in `[workspace.package]`, used by every crate, so no crate can be published by mistake before R01.
- **CI** in `.github/workflows/ci.yml`, run for every push and pull request:
  - the `check` job on Windows, Linux and macOS: format, clippy, build, test, and a new project from `dovetail new` (M05);
  - the `macro-errors` job: the macro error message snapshots (M03);
  - the `window-self-test` job on all three systems: the counter and window self-tests, and the GPU tests. Linux runs them under Xvfb with software Vulkan.
- **The licence:** MIT OR Apache-2.0 ([D0005](../../decisions/D0005-permissive-licence.md), Accepted), with `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE` in the root.

## Results

| # | Criterion | Result | Evidence |
|---|---|---|---|
| 1 | The root is a virtual workspace and every crate builds | Met | `cargo build --workspace` in CI on all three systems |
| 2 | `cargo test` and `cargo clippy -- -D warnings` pass | Met | The `check` job's Test and Lint steps, on all three systems |
| 3 | CI builds, tests and lints on all three systems for every pull request, and all three pass | Met | The `CI_Fixes` branch passed on Windows, Linux and macOS on 2026-11-25, and was merged into `main` |
| 4 | D0005 Accepted, licence files in the root, every crate states its licence | Met | D0005 is Accepted; `license.workspace = true` in every crate |
| 5 | The licences of winit, wgpu and AccessKit are recorded | Met | [D0005](../../decisions/D0005-permissive-licence.md) |
| 6 | The crate name and domain checks are recorded | Met | Below |

Name and domain checks, on 2026-11-20:

- `dovetail` is taken on crates.io by an unrelated crate (version 2.0.1). The facade is called `dovetail-ui` instead, with library name `dovetail` ([D0006](../../decisions/D0006-facade-crate-name.md)).
- `dovetail-core`, `dovetail-macros`, `dovetail-layout`, `dovetail-render`, `dovetail-desktop`, `dovetail-controls` and `dovetail-cli` were free.
- `dovetailui.dev` is not registered: its RDAP lookup at the `.dev` registry returns "not found". Whether it can be bought is not confirmed.

## Changes from the plan

- **CI ran much later than planned.** Step 7 wrote the workflow while the repository had no remote, so it first ran when the repository went to GitHub. Making it pass took these fixes:
  - **The Rust version:** CI used the latest stable Rust, which had new clippy lints. The version is now pinned in `rust-toolchain.toml` and in CI. On Windows, rustup's default host must be MSVC for the pin to work.
  - **Line endings:** Windows runners checked files out with CRLF line endings, which rustfmt rejects. `.gitattributes` fixes this.
  - **Linux libraries:** winit loads the X11 and keyboard libraries when it starts, so CI installs them.
  - **Time limits:** macOS has no `timeout` command, so steps use `timeout-minutes`.
  - **Tests that depended on timing or fonts:** the media tests now wait for frames, the sharpness test needs fewer solid pixels, and the window self-test asks for a frame again after restoring.
- **Unused dependencies:** `dovetail-controls` no longer depends on `dovetail-layout` outside its tests, and `dovetail-docking` no longer depends on `dovetail-controls`.
- **Name reservation:** crate names were not reserved by publishing empty versions. The publish guard stops publishing until R01.

## Known issues

- [B0007](../../bugs/B0007-gpu-tests-crash-on-linux-ci.md): GPU tests sometimes crashed with signal 11 on the Linux runner. CI now runs them one at a time on Linux, and the bug stays open until the cause is known.

## Measurements

| What | Platform | Result |
|---|---|---|
| Workspace tests | Windows, Linux, macOS | All pass in CI on 2026-11-25 |
| Solid pixels in the sharpness test at 100% | Windows / Linux, macOS | 57 / 18 |

## Follow-up

- **Find the cause of [B0007](../../bugs/B0007-gpu-tests-crash-on-linux-ci.md)**, then run the Linux GPU tests in parallel again.
- **Upgrade the pinned Rust version** only as a planned step, as [CONTRIBUTING.md](../../../CONTRIBUTING.md#setup) describes.
