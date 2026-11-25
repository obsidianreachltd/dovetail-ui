# M02 — Component and state core

| Field | Value |
|---|---|
| ID | M02 |
| Status | Done |
| Depends on | M00 |
| Plan | [M02 plan](../plans/M02-component-and-state-core.md) |
| Report | [M02 report](../reports/M02-component-and-state-core.md) |

## Summary

Build the component model in `dovetail-core`: identity, properties, events, reactive state and clean-up, usable from plain Rust without a macro.

## Why it matters

The macro, layout, accessibility and commands all build on this model. The reactive handle design shapes the whole public API and is hard to change later.

## What it includes

- Prototypes of at least two reactive handle designs, and a decision.
- Components with typed properties, events and children.
- Local state for each component instance.
- Stable keys for lists.
- Update scheduling, and marking what needs to be laid out, painted or made accessible again.
- Removing components and cleaning up their subscriptions.

## What it leaves out

- The `ui!` macro ([M03](M03-declarative-macro.md)).
- Drawing anything on screen ([M04](M04-layout-and-painting.md)).

## Done when

1. A prototype report compares at least two reactive handle designs, and the choice is recorded as a decision.
2. Components with typed properties and children can be built in plain Rust.
3. A test shows that changing state marks only the affected components for update.
4. A test shows that keyed children keep their state when a list is reordered.
5. A test shows that removing a component drops its state and subscriptions, with nothing left behind.
6. An event from a child reaches a handler in its parent with a typed value.
