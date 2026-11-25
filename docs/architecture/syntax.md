# Declarative syntax

UI is declared inside Rust with two procedural macros from `dovetail-macros` ([M03](../milestones/overviews/M03-declarative-macro.md)):

- `#[component]` turns a function into a component.
- `ui!` declares a view.

Both are re-exported by `dovetail::prelude`.

## Example

This is the agreed version of the original example. It compiles and runs as a test (`crates/dovetail-ui/tests/ui_macro.rs`). The only change from the first draft is the return type: `-> View`, not `-> impl View`.

```rust
use dovetail::prelude::*;
use dovetail_controls::{Button, Column, Text};

#[component]
fn Counter() -> View {
    let count = use_state(|| 0);

    ui! {
        Column {
            Text {
                value: count.map(|value| format!("Count: {value}")),
            }

            Button {
                text: "Increment",
                on_click: move |_| count.update(|value| *value += 1),
            }
        }
    }
}
```

`count` is a `Signal`, a small `Copy` handle ([D0010](../decisions/D0010-copy-arena-handles.md)). That is why it can be moved into the closure and still used in `count.map(...)`. The [prototype report](reactive-handles-prototype.md) compares this with a design where that does not compile.

## Components

A `#[component]` function's parameters are its properties.

| Written | Meaning |
|---|---|
| `label: String` | Required. `ui!` must give it, as a `String`. |
| `#[prop(default)] count: u32` | Optional. Left out, it is `u32::default()`. |
| `#[prop(default = "grey")] colour: &'static str` | Optional, with this value when left out. |
| `#[prop(into)] text: String` | Takes anything that converts into the type, such as `"Go"`. It can be combined with `default`. |
| `on_click: Callback<()>` | Takes a closure, `move \|_\| ...`. |
| `children: Children` | The nodes written inside the component's braces. It defaults to none. |

The body builds the component's view. It may call `use_state`, always in the same order. It runs again when state it read changes, or when its properties change.

The macro generates three things:

- a component type with the function's name, for example `Counter`
- a `CounterProps` struct with one public field per property
- a hidden builder that checks required properties when compiling

## `ui!` grammar

```text
nodes     = node*
node      = component | "text" | { expression } | if | for
component = Path { (property: expression ,?)* nodes }
if        = if condition { nodes } (else if ... | else { nodes })?
for       = for pattern in expression (, key = expression)? { node }
```

| Written | Expands to (M02 calls) |
|---|---|
| `Button { text: "Go" }` | `View::component::<Button>(ButtonProps::__build(ButtonProps::__builder().text("Go")))` |
| Nodes inside a component | `.children(vec![...])` |
| `"text"` | `View::text("text")` |
| `{ expression }` | The expression as a view: a `String`, `&str` or `View` |
| `if a { .. } else { .. }` | `if a { .. } else { .. }`, with no `else` giving `View::Empty` |
| `for x in xs, key = x.id { node }` | `View::fragment(xs.into_iter().map(\|x\| node.keyed(x.id)))` |
| Several nodes | `View::fragment([...])` |

Nothing is parsed at run time. A test checks that the expansion equals the same view written by hand.

A `for` without `key` matches items by position, so an item's state stays at its position when the list is reordered. Give a `key` for lists that change order.

## Error messages

Errors point at the declaration in `ui!`, not at generated code. Each one below is locked by a compile-fail test in `crates/dovetail-macros/tests/fail`:

| Mistake | Message |
|---|---|
| A required property is left out | `` `Label` needs the property `text` ``, with a note on how to give it |
| An unknown property | ``no method named `colour` found`` at the property name |
| A wrong type | `` expected `u32`, found `&str` `` at the value |
| A property given twice | ``the property `text` is given more than once`` |
| A misspelt component | ``cannot find type `Lable` `` with a suggestion |
| `children:` as a property | ``write children as nodes inside the braces`` |
| A `for` with more than one node | ``the body of a `for` must be exactly one node`` |
| An unknown `#[prop(...)]` option | ``unknown property option`` |

## Limits

- Components cannot have generic parameters yet.
- A component path that uses the surrounding function's generic parameters is not supported inside `ui!`.
- A `Callback` property takes a closure. To pass an existing `Callback`, write `move |v| callback.call(v)`.
