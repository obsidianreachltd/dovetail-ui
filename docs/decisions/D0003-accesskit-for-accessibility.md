# D0003 — Use AccessKit for accessibility

| Field | Value |
|---|---|
| ID | D0003 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

Each operating system has its own accessibility interface, used by screen readers and other assistive tools. Writing all three connections ourselves would be a lot of work and easy to get wrong.

## Decision

Dovetail uses AccessKit to connect to the accessibility services of each platform. Dovetail's job is to build a correct accessibility tree from its components, and to keep that tree up to date.

## Consequences

- Each component can describe its role, name, state and the actions it supports.
- The accessibility tree must be updated when components change, and must match keyboard focus.
- Screen readers still need manual testing on each OS ([M07](../milestones/overviews/M07-accessibility.md)).

## Alternatives considered

- **Write our own platform connections:** full control, but a large amount of platform-specific work.
