# D0015 — AccessKit through its winit adapter, pinned

| Field | Value |
|---|---|
| ID | D0015 |
| Status | Accepted |
| Date | 2026-11-22 |

## Context

[D0003](D0003-accesskit-for-accessibility.md) chose AccessKit for accessibility, but not how to connect it, which version to use, or what comes with it. M07 needs this settled.

AccessKit has a core crate (`accesskit`) and an adapter for each platform: Windows, macOS, and Unix (AT-SPI over D-Bus). It also has an adapter for winit (`accesskit_winit`) that picks the right platform adapter and wires it to a winit window.

Checked on 2026-11-22 against our pinned winit 0.30.13, the number of crates in the build grows:

| System | winit alone | With AccessKit |
|---|---|---|
| Windows | 13 | 37 |
| macOS | 29 | 37 |
| Linux | 69 | 141 |

On Linux the extra crates are mostly zbus (D-Bus), an async runtime (async-io) and serde, which the accessibility bus needs.

## Decision

`dovetail-desktop` uses `accesskit_winit` `=0.34.2` and `accesskit` `=0.25.2`, pinned to exact versions in the root `Cargo.toml` like winit and wgpu. The default features stay on, so Linux uses the async-io runtime.

- Only `dovetail-desktop` depends on AccessKit. `dovetail-core` describes roles, names and states in its own types, and the desktop crate turns them into AccessKit nodes. `dovetail-core` keeps no dependencies.
- The crates AccessKit itself needs come with it under D0003 and this decision. They need no decision of their own.
- Upgrading AccessKit is a planned step, as for winit and wgpu, because its adapter must match the winit version.

## Consequences

- Screen readers on all three systems can use Dovetail windows, through AccessKit's maintained adapters.
- Builds on Linux take longer and pull in many more crates. Their licences must stay compatible with ours. Checked with `cargo metadata` on 2026-11-22, every crate in the build is under permissive terms (MIT, Apache-2.0, BSD, ISC, Zlib or Unlicense, mostly MIT OR Apache-2.0). The few that offer LGPL offer it only as one choice alongside MIT or Apache-2.0. `accesskit_winit` itself is Apache-2.0.
- The event loop must carry AccessKit's events, so it gets a user event type.
- A window must be created hidden and shown after its adapter exists, as `accesskit_winit` requires.

## Alternatives considered

- **The platform adapters directly** (`accesskit_windows`, `accesskit_macos`, `accesskit_unix`): the same dependencies, with more glue code of our own. Only useful if Dovetail stops using winit.
- **Windows and macOS only for now**, without the Linux adapter: 72 fewer crates on Linux, but no accessibility there, and M07 needs Orca to read the examples.
- **The tokio runtime on Linux:** only worth it for applications that already use tokio. It can be offered as a feature later.
