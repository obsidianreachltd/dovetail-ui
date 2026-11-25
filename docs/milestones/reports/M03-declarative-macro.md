# M03 report — Declarative macro

| Field | Value |
|---|---|
| ID | M03 |
| Finished on | 2026-11-20 |
| Commit or tag | Not committed yet |
| Plan | [M03 plan](../plans/M03-declarative-macro.md) |

## Summary

`#[component]` and `ui!` in `dovetail-macros` expand declarations into plain M02 calls at compile time. The example in the syntax document compiles as agreed. Every mistake that was tested gives an error at the user's declaration, and each error is locked by a compile-fail test. All five criteria are met, on Windows. CI has not run yet.

## What was delivered

- `dovetail-macros`:
  - `#[component]` (`component.rs`): a component type, a `<Name>Props` struct, and a typed builder that checks required properties.
  - `ui!` (`ui.rs`): components, text, expressions, `if`/`else` and keyed `for`.
- `dovetail-core`: a free `use_state` for the current build, and the hidden `__private` support module.
- `dovetail-controls`: `Column`, `Text` and `Button`, written with `#[component]`. They have their properties but no layout or painting yet (M04).
- `dovetail` facade: re-exports the core API at its root and in `prelude`, with the macros.
- Tests:
  - 5 pass tests in `crates/dovetail-ui/tests/ui_macro.rs`
  - 1 facade doc test
  - 8 compile-fail cases in `crates/dovetail-macros/tests/fail`
- Decision [D0011](../../decisions/D0011-macro-dependencies.md): syn, quote, proc-macro2 and trybuild.
- [Syntax](../../architecture/syntax.md) rewritten with the grammar, the expansion and the error messages.
- CI: a pinned job for the macro error messages.

## Results

| # | Criterion | Result | Evidence |
|---|---|---|---|
| 1 | The Counter example, or its agreed version, compiles | Met | `counter_example_compiles_and_counts`. The only change is `-> View` instead of `-> impl View`. |
| 2 | Generated code uses only the M02 API, with nothing parsed at run time | Met | `expansion_equals_hand_written_code` |
| 3 | A component from another crate works after an ordinary `use` | Met | `use dovetail_controls::Button;` in the pass tests |
| 4 | Unknown, wrong-type and missing properties give errors at the declaration, locked by tests | Met | `unknown_property`, `wrong_type` and `missing_property` `.stderr` files, plus 5 more cases |
| 5 | Conditional content and keyed lists can be written in `ui!` | Met | `conditional_content_and_keyed_lists`, `keyed_for_keeps_instances_when_reordered` |

## Changes from the plan

- **Qualified paths:** a struct literal through a qualified path is not stable. A builder replaces the struct literal and `..Default::default()`, which also cannot leave required properties out.
- **`__build`:** it became an associated function. As a method, a missing property gave "method exists but its trait bounds were not satisfied". Now it gives "`Label` needs the property `text`".
- **Property types:** they are exact by default, with `#[prop(into)]` to opt in. An `Into` setter could not infer an integer literal such as `3`.
- **Toolchain pin:** the `rust-toolchain.toml` pin was removed (see the plan's risks). The compile-fail tests run in a separate CI job pinned to 1.94.0.
- **Clippy:** it flagged three `if`s that could be collapsed, in `prototypes/reactive-handles`. They were fixed.

## Known issues

None recorded as bugs. Two error messages could read better:

- An unknown property names the hidden builder type (``no method named `colour` found for struct `__LabelPropsBuilder<...>` ``).
- A missing property also shows the internal trait name in its help line.

## Measurements

| What | Platform | Result |
|---|---|---|
| Compile-fail suite (8 cases), warm | Windows | 0.9–1.6 s |
| Workspace tests passing | Windows | 50 (3 ignored: GPU and doc examples) |

## Follow-up

- **M04:** give `Column`, `Text` and `Button` layout and painting, and call `on_click` on a press.
- **Later:**
  - Components with generic parameters.
  - A clearer unknown-property message, which could list the known properties from `ui!`.
- **M05:** `dovetail new` templates use `dovetail::prelude::*` and `#[component]`.
- Run the CI jobs once the repository has a remote.
