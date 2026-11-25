# D0011 — Dependencies for the macros

| Field | Value |
|---|---|
| ID | D0011 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

M03 builds `#[component]` and `ui!` as procedural macros. [D0002](D0002-build-own-stack.md) allows only winit, wgpu and AccessKit as foundation libraries, and says anything else needs a decision. A procedural macro must parse Rust tokens and write new ones, and its errors must keep the user's spans.

## Decision

`dovetail-macros` uses:

| Crate | Version | Use | Licence |
|---|---|---|---|
| `syn` | 3 | Parse Rust syntax | MIT OR Apache-2.0 |
| `quote` | 1 | Write Rust tokens | MIT OR Apache-2.0 |
| `proc-macro2` | 1 | Tokens and spans outside the compiler | MIT OR Apache-2.0 |
| `trybuild` | 1 (dev only) | Compile-fail tests that lock error messages | MIT OR Apache-2.0 |

- These run only while compiling. None of them is linked into an application.
- `trybuild` 1.0.125 needs Rust 1.88, so the workspace `rust-version` goes up from 1.87 to 1.88.

## Consequences

- The macros can parse full Rust expressions inside `ui!`, and errors point at the user's code.
- Compile times for crates that use the macros go up a little, by the time to build `syn` once.
- The licences fit [D0005](D0005-permissive-licence.md).

## Alternatives considered

- **Parse tokens by hand with only `proc_macro`:** no dependencies. But expressions inside `ui!` (closures, generics, blocks) are hard to parse by hand, and keeping spans correct is easy to get wrong.
- **syn 2:** older and still maintained. It is the fallback if syn 3 lacks something.
