# D0006 — Name of the facade crate

| Field | Value |
|---|---|
| ID | D0006 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

The M00 name check on 2026-11-20 found that `dovetail` is already taken on crates.io. It is version 2.0.1 of an unrelated crate, last updated 2024-08-10. The other seven names, `dovetail-core`, `dovetail-macros`, `dovetail-layout`, `dovetail-render`, `dovetail-desktop`, `dovetail-controls` and `dovetail-cli`, are free.

The facade crate cannot be published as `dovetail`.

## Decision

The facade package is published as `dovetail-ui`, in `crates/dovetail-ui`. Its library name stays `dovetail` with `[lib] name = "dovetail"`. Applications still write `use dovetail::prelude::*;`, and only the dependency line changes:

```toml
[dependencies]
dovetail-ui = "0.1"
```

## Consequences

- The `use dovetail::...` syntax and the `dovetail` command stay the same.
- The package name and the library name differ, which needs a line in the getting-started documents.
- The other crates keep the `dovetail-` prefix.

## Alternatives considered

- **Ask the owner of `dovetail` to transfer the name:** possible, but slow and not certain.
- **Rename the whole prefix:** avoids the difference between package and library names, but every crate and document changes.
