# M00 — Workspace foundation

| Field | Value |
|---|---|
| ID | M00 |
| Status | In progress |
| Depends on | — |
| Plan | [M00 plan](../plans/M00-workspace-foundation.md) |
| Report | — |

## Summary

Turn the repository into a Cargo workspace with the first crates, CI on all three operating systems, and the licence chosen.

## Why it matters

Every later milestone adds code to these crates and relies on CI to prove it works on Windows, Linux and macOS. The licence has to be settled before code is shared.

## What it includes

- A virtual workspace at the root, with shared version, edition and dependency settings.
- Empty crates for the [first subset](../../architecture/crates.md#first-subset).
- Shared formatting and lint settings.
- CI that builds, tests and lints on Windows, Linux and macOS.
- Choosing the licence ([D0005](../../decisions/D0005-permissive-licence.md)) and checking the foundation licences.
- Checking that the names `dovetail-*` and `dovetailui.dev` are free.

## What it leaves out

- Any window, rendering or UI code ([M01](M01-window-and-gpu.md) onwards).
- Publishing crates ([R01](R01-first-release.md)).

## Done when

1. The root `Cargo.toml` is a virtual workspace and all crates build with `cargo build --workspace`.
2. `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` pass.
3. CI runs build, test and lint on Windows, Linux and macOS for every pull request, and all three pass.
4. D0005 is Accepted, the licence files are in the root, and every crate states its licence.
5. The licences of winit, wgpu and AccessKit are recorded in D0005.
6. The results of the crate name and domain checks are recorded in the report.
