# D0004 — One repository as a Cargo workspace

| Field | Value |
|---|---|
| ID | D0004 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

The framework is split into several crates. They could live in one repository or in several.

## Decision

All crates live in one repository, organised as a Cargo workspace. The layout is shown in [Architecture](../architecture/overview.md#repository-layout).

## Consequences

- Changes that touch several crates can be made and tested together.
- Versions and dependencies are managed in one place.
- Each crate must still build and publish on its own.
- All crates share one version and are released together while the API changes.

## Alternatives considered

- **One repository per crate:** each crate is cleanly separated, but changes across crates are slow and versions are hard to keep in step.
