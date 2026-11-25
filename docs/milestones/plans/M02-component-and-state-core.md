# M02 plan — Component and state core

| Field | Value |
|---|---|
| ID | M02 |
| Status | Done |
| Overview | [M02 overview](../overviews/M02-component-and-state-core.md) |
| Last updated | 2026-11-20 |

## Goal

Build the component model in `dovetail-core`: identity, properties, events, reactive state and clean-up, usable from plain Rust without a macro.

## Starting point

- `dovetail-core` is an empty crate with no dependencies, and it must stay that way.
- M01 gives windows and drawing with one clear colour. M02 does not draw, so it does not depend on M01 code.
- The `ui!` example in [syntax](../../architecture/syntax.md) uses `count` both in `count.map(...)` and in a `move` closure. Whatever handle is chosen must make that work.

## Steps

1. [x] Build prototype A, **arena handles**. State lives in a runtime owned by the component tree, and the handle is a small `Copy` value (an index and a generation). See `prototypes/reactive-handles/src/arena.rs`.
2. [x] Build prototype B, **shared handles**. State lives in `Rc<RefCell<T>>`, and the handle is `Clone` but not `Copy`. See `prototypes/reactive-handles/src/shared.rs`.
3. [x] Run the same scenario tests on both: the `ui!` example written by hand, dependency tracking, clean-up, a handle used after its component is removed, and a reference cycle. There are 13 tests and 3 doc tests, and all pass.
4. [x] Measure both: handle size, the time to create and drop 10,000 components, the time for 1,000,000 updates, and the time for 1,000,000 tracked reads.
5. [x] Write the [prototype report](../../architecture/reactive-handles-prototype.md), and record the choice as decision D0010. [D0010](../../decisions/D0010-copy-arena-handles.md) chose A, Copy arena handles, and was accepted on 2026-11-20.
6. [x] Build component identity: a component tree in which each node has an ID that is never reused. These are `Tree` and `ComponentId`.
7. [x] Build typed properties and children: a `Component` trait with its own `Props` type, built in plain Rust. `Props` must be `PartialEq`, and children are a `Children` property.
8. [x] Build local state for each component instance, using the chosen handle. `BuildContext::use_state` returns the same `Signal` in every build, in call order.
9. [x] Build typed events: a child sends a value, and a handler in its parent receives it. A `Callback<T>` equals another only if it is the same handler.
10. [x] Build keyed children: when a list changes, items with the same key keep their instance and state (`View::keyed`). Children without a key match by position among children of the same type.
11. [x] Build update scheduling: a state change marks the components that read it, and each mark says whether layout, painting or accessibility must be redone.
    - `Tree::update` builds marked components, parents first, and returns an `Update`.
    - A child is built again only if its properties changed.
    - Until M04 and M07, any change to a component's own view gives `Invalidation::ALL`.
12. [x] Build removal: removing a component drops its state, subscriptions and children. State drops before properties and handlers. `runtime_stats` counts what is left.
13. [x] Write tests for "Done when" items 2–6, plus a test that nothing is left behind. There are 9 integration tests in `crates/dovetail-core/tests/component_tree.rs`, 6 unit tests and 1 doc test.

## Prototypes and open decisions

- **Reactive handle design:** decided, A, arena handles ([D0010](../../decisions/D0010-copy-arena-handles.md)).
- **Re-entrant runtime use:** in prototype A, code inside `update` or `map` cannot use the runtime, because it is borrowed. The real version must release the borrow before it calls user code (D0010).
- **How reads are tracked:** automatically, while a component builds, or by explicit subscription. Both prototypes track automatically during a component's build.
- **Threads:** the component tree runs on the UI thread only. Neither prototype is `Send`. Long-running work comes in M11.

## Risks

- **The handle choice is hard to undo.** Prototype first, and do not start step 6 until D0010 is accepted.
- **Arena handles can outlive their component.** Using one must give a clear error, never undefined behaviour.
- **Shared handles can form reference cycles** that never drop. Tests must look for leaks.

## Testing

- Unit tests in `dovetail-core` for every step from 6 to 12.
- The prototypes in `prototypes/reactive-handles` keep their scenario tests and measurements, so the report can be checked again.
- Measurements are recorded in the prototype report and in the M02 completion report.

## Done when

Copied from the overview.

1. A prototype report compares at least two reactive handle designs, and the choice is recorded as a decision.
2. Components with typed properties and children can be built in plain Rust.
3. A test shows that changing state marks only the affected components for update.
4. A test shows that keyed children keep their state when a list is reordered.
5. A test shows that removing a component drops its state and subscriptions, with nothing left behind.
6. An event from a child reaches a handler in its parent with a typed value.
