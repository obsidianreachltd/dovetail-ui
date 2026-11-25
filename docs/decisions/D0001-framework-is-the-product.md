# D0001 — The framework is the product

| Field | Value |
|---|---|
| ID | D0001 |
| Status | Accepted |
| Date | 2026-11-20 |

## Context

The project began with a PCB design tool in mind. We had to decide which is the real goal: the framework or the PCB tool.

## Decision

Dovetail UI is the product. It is a framework that other developers build applications with. The PCB-like editor is a validation application that proves the framework works.

## Consequences

- The public API, its documentation, and its stability matter from the start.
- Built-in and third-party components must use the same contracts.
- Features are judged by whether they help framework users in general, not only the PCB editor.
- The validation editor stays small and is not released as a product.

## Alternatives considered

- **The PCB tool is the product:** this would favour reusing existing frameworks and shipping sooner, but it would not produce a framework for others.
- **Both with equal weight:** this would split the focus too early.
