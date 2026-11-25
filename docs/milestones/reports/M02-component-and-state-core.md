# M02 report — Component and state core

| Field | Value |
|---|---|
| ID | M02 |
| Finished on | 2026-11-20 |
| Commit or tag | Not committed yet |
| Plan | [M02 plan](../plans/M02-component-and-state-core.md) |

## Summary

`dovetail-core` now has the component model in plain Rust:

- components with typed properties and children
- reactive state with `Copy` handles ([D0010](../../decisions/D0010-copy-arena-handles.md))
- typed events, keyed lists, and updates that build only what changed
- removal that leaves nothing behind

All six criteria are met. They were checked on Windows. CI has not run yet, because the repository has no remote.

## What was delivered

- `prototypes/reactive-handles`: two prototypes, their scenario tests and measurements, and the [prototype report](../../architecture/reactive-handles-prototype.md).
- Decision [D0010](../../decisions/D0010-copy-arena-handles.md): `Copy` arena handles.
- `dovetail-core`, which still has no dependencies:
  - `Signal`, `create_signal`, `StateError` and `runtime_stats`, in `reactive.rs`.
  - `Component`, `View`, `Children`, `Key` and `Callback`, in `view.rs`.
  - `Tree`, `BuildContext`, `Update` and `Invalidation`, in `tree.rs`.
- Tests: 9 integration tests in `crates/dovetail-core/tests/component_tree.rs`, 6 unit tests and 1 doc test.
- `crates/dovetail-core/examples/measure_tree.rs`, which measures the tree.

## Results

| # | Criterion | Result | Evidence |
|---|---|---|---|
| 1 | Prototype of two handle designs, choice recorded | Met | Prototype report; D0010 accepted on 2026-11-20 |
| 2 | Typed properties and children in plain Rust | Met | `typed_properties_and_children_in_plain_rust` |
| 3 | Changing state marks only the affected components | Met | `changing_state_marks_only_the_components_that_read_it`, `a_change_that_gives_the_same_view_invalidates_nothing` |
| 4 | Keyed children keep their state when reordered | Met | `keyed_children_keep_their_instance_and_state_when_reordered`; `unkeyed_children_are_matched_by_position` shows the difference |
| 5 | Removing a component leaves nothing behind | Met | `removing_a_component_drops_its_state_and_subscriptions` (state dropped, `runtime_stats` back to zero) |
| 6 | A typed event from a child reaches its parent | Met | `a_typed_event_from_a_child_reaches_its_parent` |

## Changes from the plan

- **Release the borrow:** the runtime no longer holds its borrow while user code runs. State values are kept in their own cells. Code inside `map` and `update` can use other signals (`user_code_may_use_other_signals`). Using a signal inside its own update gives `StateError::InUse` instead of a panic.
- **`Invalidation` is coarse:** it is always all-or-nothing until M04 and M07 give layout, painting and accessibility something to tell apart.
- **Duplicate-key check:** measuring found that it was quadratic. A list of 10,000 items took 117 ms to rebuild with no changes. It now uses a set, and the same rebuild takes 7 ms.
- **Clear errors for mistakes:** `use_state` called in a different order and duplicate keys both panic with a clear message. Each has a test.

## Known issues

None.

## Measurements

These are release builds on Windows 11 with an x86-64 CPU. Each figure is the median of 7 runs, and the whole set was run twice. Run `cargo run --release -p dovetail-core --example measure_tree`.

| What | Platform | Result |
|---|---|---|
| Mount a list of 10,000 keyed items | Windows | 17 ms |
| Reverse the list (every instance kept) | Windows | 7 ms |
| Rebuild the list with the same items | Windows | 7 ms |
| Unmount the list | Windows | 9 ms |
| Handle size, `Signal<T>` | All | 8 bytes |

## Follow-up

- **M03:** `ui!` and `#[component]` generate calls to this API.
- **M04:**
  - Connect `Tree::update` to `dovetail-desktop`, so a marked component asks its window to redraw.
  - Split `Invalidation` into layout, paint and accessibility.
  - Measure again with real layout.
- **M07:** use the accessibility flag.
- **M11:** long-running work must send results back to the UI thread, because state is not `Send`.
- Run the tests on Linux and macOS once CI has a remote.
