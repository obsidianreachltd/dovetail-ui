# M00 plan — Workspace foundation

| Field | Value |
|---|---|
| ID | M00 |
| Status | In progress |
| Overview | [M00 overview](../overviews/M00-workspace-foundation.md) |
| Last updated | 2026-11-20 |

## Goal

Turn the repository into a Cargo workspace with the first crates, CI on all three operating systems, and the licence chosen.

## Starting point

- The repository holds one crate, `dovetail`, made by `cargo new`. It uses edition 2024, has no dependencies, and its `src/main.rs` prints "Hello, world!".
- There are no commits yet.
- Documentation is in `docs/`.

## Steps

1. [x] Change the root `Cargo.toml` into a virtual workspace, with `resolver = "3"`, `[workspace.package]` (version, edition 2024, licence) and `[workspace.dependencies]`.
2. [x] Remove the root `src/main.rs`.
3. [x] Create the library crates `dovetail`, `dovetail-core`, `dovetail-macros` (as a procedural macro crate), `dovetail-layout`, `dovetail-render`, `dovetail-desktop` and `dovetail-controls` in `crates/`. The facade was renamed to `dovetail-ui` afterwards, keeping the library name `dovetail` ([D0006](../../decisions/D0006-facade-crate-name.md)).
4. [x] Create `dovetail-cli` as a binary crate that installs a command called `dovetail`.
5. [x] Set the dependencies between crates to match the [dependency graph](../../architecture/crates.md#dependency-graph).
6. [x] Add `[workspace.lints]`, `rustfmt.toml`, and make sure `target/` is in `.gitignore`.
7. [x] Add a CI workflow that runs `cargo fmt --check`, `cargo clippy` and `cargo test` on Windows, Linux and macOS. Written as `.github/workflows/ci.yml`, but not run yet because the repository has no remote.
8. [x] Check and record the licences of winit, wgpu, AccessKit and their dependencies in [D0005](../../decisions/D0005-permissive-licence.md). Only the crates themselves are checked so far. Their dependencies are checked in M01.
9. [x] Choose the licence, mark D0005 as Accepted, add the licence files, and add a `license` entry to every crate. Chosen: MIT OR Apache-2.0.
10. [x] Check whether the `dovetail-*` names are free on crates.io and whether `dovetailui.dev` is free, and record the results. Checked on 2026-11-20:
    - `dovetail` is taken by an unrelated crate (version 2.0.1). See [D0006](../../decisions/D0006-facade-crate-name.md).
    - The other seven crate names are free.
    - `dovetailui.dev` is not registered. Its RDAP lookup at the `.dev` registry returns "not found". Whether it can be bought is not confirmed.

## Prototypes and open decisions

- **Licence choice**, in D0005.
- **Name reservation**: whether to reserve crate names by publishing empty versions. Record this as a new decision if we do.

## Risks

- **Names already taken on crates.io.** Pick a new prefix early, before code refers to it.
- **CI minutes on macOS can be limited.** Run full checks on pull requests and a smaller set on drafts.
- **winit and wgpu need a newer Rust than the workspace sets.** winit needs 1.86 and wgpu needs 1.87, but `rust-version` is 1.85. Raise `rust-version` when they are added in M01.

## Testing

- CI itself, on Windows, Linux and macOS.
- `cargo build --workspace` and `cargo test --workspace` run on a clean clone.

## Done when

Copied from the overview.

1. The root `Cargo.toml` is a virtual workspace and all crates build with `cargo build --workspace`.
2. `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` pass.
3. CI runs build, test and lint on Windows, Linux and macOS for every pull request, and all three pass.
4. D0005 is Accepted, the licence files are in the root, and every crate states its licence.
5. The licences of winit, wgpu and AccessKit are recorded in D0005.
6. The results of the crate name and domain checks are recorded in the report.
