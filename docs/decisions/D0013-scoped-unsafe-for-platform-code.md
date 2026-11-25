# D0013 — `unsafe` only in marked platform modules

| Field | Value |
|---|---|
| ID | D0013 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

The workspace denies `unsafe` code (`unsafe_code = "deny"`). M06 needs the system clipboard, which winit does not provide. Each operating system offers it only through its own interface: Win32 on Windows, AppKit on macOS, and the X11 and Wayland protocols on Linux. Calling those interfaces from Rust needs `unsafe`.

[D0002](D0002-build-own-stack.md) keeps foundation libraries to winit, wgpu and AccessKit, so a clipboard library would need its own decision.

## Decision

Dovetail calls platform interfaces with its own code. `unsafe` is allowed only:

- in modules under `crates/dovetail-desktop/src/platform/`, one for each OS;
- for platform features that winit, wgpu and AccessKit do not provide. The first is the clipboard.

Each such module:

- starts with `#![allow(unsafe_code)]` and a comment saying what it wraps;
- has a `// SAFETY:` comment on every `unsafe` block, saying why the call is sound;
- offers only a safe interface to the rest of the crate.

The workspace lint stays `deny`. Everywhere else, `unsafe` is still an error.

## Consequences

- There are no extra dependencies. The clipboard code is Dovetail's own, and it can be tested and changed freely.
- There is more code to write and review for each OS, and macOS, X11 and Wayland can only be tested on those systems.
- Reviewers know exactly where to look for `unsafe` (`git grep unsafe -- crates/dovetail-desktop/src/platform`).
- New platform features that need `unsafe` go in the same modules. A new place for `unsafe` needs a new decision.

## Alternatives considered

- **A clipboard library, such as arboard:** less code, but one more dependency against D0002, and its own `unsafe` still runs inside Dovetail.
- **`unsafe` anywhere it is needed:** simpler, but hard to review and easy to misuse.
