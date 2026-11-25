# D0005 — Use a permissive licence

| Field | Value |
|---|---|
| ID | D0005 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

Dovetail UI is an open-source project owned by Obsidian Reach Ltd. Anyone must be free to use, copy, change, share and contribute to it, including in closed-source commercial applications. Its licence, and the licences of its dependencies, must allow that.

## Decision

Dovetail is dual licensed as **MIT OR Apache-2.0**. Users may choose either licence. This was chosen in [M00](../milestones/overviews/M00-workspace-foundation.md).

- The licence texts are in `LICENSE-MIT` and `LICENSE-APACHE` at the repository root.
- The workspace sets `license = "MIT OR Apache-2.0"` and `authors = ["Obsidian Reach Ltd"]`, and every crate inherits both.
- The copyright is "Obsidian Reach Ltd and the Dovetail UI contributors". It is stated in `LICENSE-MIT` and `NOTICE`.
- Contributors keep the copyright of their contributions and license them under the same terms ([CONTRIBUTING.md](../../CONTRIBUTING.md#licence)).

The licences of the foundation libraries are:

| Library | Licence | Checked on |
|---|---|---|
| winit 0.31.0-beta.4 | Apache-2.0 | 2026-11-20 |
| wgpu 30.0.2 | MIT OR Apache-2.0 | 2026-11-20 |
| accesskit 0.25.2 | MIT OR Apache-2.0 | 2026-11-20 |
| accesskit_winit 0.34.2 | Apache-2.0 | 2026-11-20 |

These are the licences of the crates themselves, from `cargo info`. All of them allow closed-source commercial use. Their dependencies were checked on 2026-11-20, when winit 0.30.13 and wgpu 30.0.2 were added in M01, using `cargo metadata` for every platform. All 254 external crates are permissive: MIT, Apache-2.0, BSD, ISC, Zlib, Unlicense or Unicode-3.0. The only one that offers LGPL also offers MIT or Apache-2.0. Repeat this check whenever a dependency is added.

## Consequences

- Every crate's `Cargo.toml` states the licence, and the licence files are in the repository root.
- New dependencies must have compatible licences.

## Alternatives considered

- **Copyleft licences** (for example GPL): these do not fit closed-source commercial use.
- **Apache-2.0 only:** includes a clear patent grant, but some projects cannot use it.
- **MIT only:** short and simple, but has no clear patent grant.
