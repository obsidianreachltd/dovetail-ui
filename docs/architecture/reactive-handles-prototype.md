# Reactive handles: prototype report

This report compares two designs for the handle to a component's reactive state, as the first step of [M02](../milestones/plans/M02-component-and-state-core.md). The choice, A, is recorded in [D0010](../decisions/D0010-copy-arena-handles.md), which was accepted on 2026-11-20.

The code is in `prototypes/reactive-handles`. It is throw-away code, but it stays in the repository so these results can be checked again.

## The two designs

Both have the same small API:

- `create_scope` makes a scope, which is one component instance.
- `build` runs a scope's build.
- `use_state` makes state owned by the scope that is building.
- Reads while building subscribe that scope.
- `update` marks the subscribed scopes.
- `dispose` removes a scope and its children.

| | A: arena handles (`arena.rs`) | B: shared handles (`shared.rs`) |
|---|---|---|
| Where state lives | A runtime for the UI thread, in numbered slots | `Rc<RefCell<T>>` |
| The handle | `Signal<T>`: slot number and generation, `Copy` | `State<T>`: an `Rc`, `Clone` but not `Copy` |
| Who owns state | The scope that made it | Every handle, plus the scope |
| Handle used after its component is removed | A clear error (`Disposed`), or a panic with a clear message | Works silently on state that nothing shows |

## Scenario results

The same scenarios run as tests on both designs, with `cargo test -p prototype-reactive-handles`. All tests pass. Each test checks what that design does, including where the design is weak.

| Scenario | A: arena | B: shared |
|---|---|---|
| The `ui!` example in [syntax](syntax.md), written exactly as in the document | Compiles | Does not compile (error E0382, use after move). It needs `let count = count.clone();` before the closure. A doc test checks both. |
| An update marks only the components that read the state | Yes | Yes |
| A rebuild forgets the reads of the build before it | Yes | Yes |
| Removing a component drops its state and its subscriptions | Yes. Nothing is left in the runtime. | Yes. Only handles held elsewhere keep state alive. |
| A handle used after its component is removed | `Err(Disposed)`. A new state in the same slot is never read by the old handle. | No error. It changes state that nothing displays. |
| State holding a closure that captures its own handle | Dropped when its component is removed | Never dropped, because the reference cycle leaks |

## Measurements

These are release builds on Windows 11 with an x86-64 CPU. Each measurement is the median of 7 runs, and the whole set was run twice with the same picture. Run `cargo run --release -p prototype-reactive-handles`.

| Measurement | A: arena | B: shared |
|---|---|---|
| Handle size | 8 bytes | 8 bytes |
| Create, build and remove one component with one state (10,000 components) | 0.61–0.69 µs | 0.82–0.92 µs |
| Update a state with one subscriber (1,000,000 updates) | 15 ns | 69 ns |
| Tracked read while building (1,000,000 reads) | 21–23 ns | 6 ns |

Neither prototype is tuned, and the differences come from simple choices, not from the designs:

- B copies its subscriber set on every update.
- A does a set insert on every read.

Both designs are thousands of times faster than a 16 ms frame needs. **Speed does not decide between them.**

## What decides

- **Writing UI code.** With A, the declared syntax works as written. With B, every closure that captures state needs a clone first. The `ui!` macro cannot add these clones, because it cannot see which names a closure captures.
- **Mistakes.** A turns a kept handle into a clear error. B hides it, and a cycle in B leaks memory without any error.
- **What A costs:**
  - A stale handle is found at run time, not at compile time.
  - All state is in a runtime for one UI thread, which M02 already assumes.
  - Code inside `update` or `map` cannot use the runtime again in this prototype, because the runtime is borrowed. The real version must release the borrow before it calls user code.

## Recommendation

Choose **A, arena handles**. See [D0010](../decisions/D0010-copy-arena-handles.md).
