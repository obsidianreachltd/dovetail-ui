# D0010 — Reactive state uses Copy arena handles

| Field | Value |
|---|---|
| ID | D0010 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

Every component's reactive state needs a handle that the component and its closures can use. The handle decides how the [`ui!` syntax](../architecture/syntax.md) is written, and it is hard to change later. M02 compared two prototypes: the [prototype report](../architecture/reactive-handles-prototype.md).

## Decision

Reactive state lives in a runtime that belongs to the UI thread. The handle, `Signal<T>`, is a slot number and a generation. It is `Copy` for every `T`.

- State belongs to the component that made it, and it is dropped when that component is removed.
- Reads while a component builds subscribe that component. Updates mark the subscribers.
- A handle used after its component is removed gives a clear error (`Disposed`), or a panic with a clear message. It never reads other state.
- The runtime is borrowed only while it is changed, never while user code runs.

## Consequences

- The `ui!` example works as written. Closures capture handles with `move` and no clones.
- Reference cycles through state cannot leak, because handles do not own state.
- Keeping a handle after its component is removed is a run-time error, not a compile-time one. Tests and error messages must make it easy to find.
- The component tree and its state run on the UI thread only. Work on other threads must send results back (M11).

## Alternatives considered

- **Shared handles, `Rc<RefCell<T>>`:** handles never dangle and need no runtime. But every captured handle needs a clone, which the macro cannot add. Stale handles fail silently, and cycles leak.
