# M03 plan — Declarative macro

| Field | Value |
|---|---|
| ID | M03 |
| Status | Done |
| Overview | [M03 overview](../overviews/M03-declarative-macro.md) |
| Last updated | 2026-11-20 |

## Goal

Add `#[component]` and `ui!` in `dovetail-macros`. They turn declarations into typed calls to the M02 core at compile time.

## Starting point

- M02 is done. `dovetail-core` has `Component`, `View`, `Children`, `Key`, `Callback`, `Signal`, `BuildContext::use_state` and `Tree`.
- `dovetail-macros` is an empty procedural macro crate with no dependencies.
- Probes on 2026-11-20 found two things:
  - A struct literal through a qualified path, `<Button as Component>::Props { .. }`, is not stable Rust (E0658, issue #86935).
  - A type alias works: `type P = <Button as Component>::Props; P { .. }`.
  - With user spans kept, rustc's own errors for an unknown property, a missing property and a wrong type point at the user's tokens.

## Steps

1. [x] Accept the macro dependencies ([D0011](../../decisions/D0011-macro-dependencies.md)) and how `#[component]` is written (step 4). Both were accepted on 2026-11-20: D0011, and a free `use_state`.
2. [x] Add `syn` 3.0.10, `quote` 1.0.47 and `proc-macro2` 1.0.107 to `dovetail-macros`, and `trybuild` 1.0.125 as a dev-dependency. Raise `rust-version` to 1.88.
3. [x] Write the `ui!` grammar and the expansion of each form into [syntax](../../architecture/syntax.md).
4. [x] Build `#[component]`. It generates:
   - the component type and a `<Name>Props` struct (`Clone`, `PartialEq`)
   - a hidden builder whose type parameters track the required properties

   `dovetail-core` gained a free `use_state`, with a hook frame for the current build.
5. [x] Build `ui!`, keeping every user span so that errors point at the declaration.
6. [x] Re-export the macros from the `dovetail` facade, in `dovetail::prelude`. The core API is re-exported at the facade's root, because the generated code refers to `::dovetail`.
7. [x] Add compile-fail tests with `trybuild`: 8 cases in `crates/dovetail-macros/tests/fail`, with their messages locked in `.stderr` files.
8. [x] Add tests that the Counter example compiles and runs, that a component from another crate works after a `use`, and that conditional content and keyed lists work. They are in `crates/dovetail-ui/tests/ui_macro.rs`.
9. [x] Add a test that the expanded code is the same as hand-written M02 code (`expansion_equals_hand_written_code`).

## Prototypes and open decisions

- **Dependencies:** decided in [D0011](../../decisions/D0011-macro-dependencies.md).
- **How `#[component]` reaches state:** decided on 2026-11-20. It uses a free `use_state`, as in the syntax example.
- **Optional properties:** a builder replaces `..Default::default()`. Struct update syntax needs every field, including required ones, so it cannot leave required properties out. A missing required property is caught by a trait bound named after it.
- **Exact type or `Into`:** a property takes its exact type, so a literal such as `3` gets the property's type. `#[prop(into)]` takes anything that converts. An `impl Into<u32>` setter could not infer `3`.
- **Generics:** components cannot have generic parameters yet (clear error).

## Risks

- **Error messages:** errors may point at generated code. Compile-fail tests lock every message, and spans are kept for every user token.
- **syn 3 is a new major version.** If it lacks something needed, use syn 2, which is still maintained.
- **Rust changes error wording** between versions. CI runs the compile-fail tests in their own job, pinned to Rust 1.94.0 on Linux. Upgrade on purpose and regenerate the snapshots.
  - A `rust-toolchain.toml` pin was tried and removed. It installs the pinned version for each machine's default host, which on this Windows machine was `windows-gnu`, and that broke the wgpu build.

## Testing

- `cargo test -p dovetail-macros` runs the `trybuild` pass and compile-fail cases.
- `cargo test -p dovetail-ui` runs a test that uses only the facade and `ui!`, as an application would.
- The expanded code is compared with M02 hand-written code in a test.

## Done when

Copied from the overview.

1. The Counter example in the syntax document, or its agreed version, compiles.
2. The generated code uses only the plain-Rust API from M02, with nothing parsed at run time.
3. A component from another crate can be used after an ordinary `use`.
4. Unknown properties, wrong types and missing properties give errors that point at the declaration, and compile-fail tests lock them.
5. Conditional content and keyed lists can be written inside `ui!`.
