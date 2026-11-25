# M03 — Declarative macro

| Field | Value |
|---|---|
| ID | M03 |
| Status | Done |
| Depends on | M02 |
| Plan | [M03 plan](../plans/M03-declarative-macro.md) |
| Report | [M03 report](../reports/M03-declarative-macro.md) |

## Summary

Add `#[component]` and `ui!` in `dovetail-macros`, which turn declarations into typed calls to the M02 core at compile time.

## Why it matters

This is how developers write UIs. Good error messages decide whether the framework is pleasant to use.

## What it includes

- `#[component]` and `ui!` ([syntax](../../architecture/syntax.md)).
- Properties, events, children, conditional content and keyed lists.
- Components from other crates, brought in with ordinary `use`.
- Error messages that point at the user's declaration.
- Compile-fail tests that pin those messages.

## What it leaves out

- Live reloading of declarations (after the first release).
- Layout and painting ([M04](M04-layout-and-painting.md)).

## Done when

1. The Counter example in the syntax document, or its agreed version, compiles.
2. The generated code uses only the plain-Rust API from M02, with nothing parsed at run time.
3. A component from another crate can be used after an ordinary `use`.
4. Unknown properties, wrong types and missing required properties give errors that point at the declaration, and compile-fail tests lock them.
5. Conditional content and keyed lists can be written inside `ui!`.
