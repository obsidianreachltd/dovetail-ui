//! Procedural macros for Dovetail UI: `#[component]` and `ui!`.
//!
//! The macros generate code that calls `dovetail-core` through the path
//! `::dovetail`, which the `dovetail-ui` facade provides. A crate that uses
//! `dovetail-core` directly adds `extern crate dovetail_core as dovetail;`.
//!
//! Nothing is parsed at run time: both macros expand to plain M02 calls.
//! See docs/architecture/syntax.md for the grammar and the expansion.

mod component;
mod ui;

use proc_macro::TokenStream;

/// Turns a function into a component.
///
/// ```ignore
/// #[component]
/// pub fn Greeting(name: String, #[prop(default)] excited: bool) -> View {
///     ui! { { format!("Hello, {name}{}", if excited { "!" } else { "" }) } }
/// }
/// ```
///
/// - The function's parameters are the component's properties. A property
///   is required unless it has `#[prop(default)]` or
///   `#[prop(default = expr)]`. A property named `children` of type
///   `Children` defaults to no children.
/// - A property takes a value of its exact type. With `#[prop(into)]` it
///   takes anything that converts into it, such as `"text"` for `String`.
/// - A property of type `Callback<T>` takes a closure.
/// - The body is the component's build. It may call `use_state`.
/// - It generates a component type with the function's name, and a
///   `<Name>Props` struct with one public field per property.
#[proc_macro_attribute]
pub fn component(
  attr: TokenStream,
  item: TokenStream,
) -> TokenStream {
  component::expand(attr.into(), item.into())
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

/// Declares a view.
///
/// ```ignore
/// ui! {
///     Column {
///         Text { value: count.map(|value| format!("Count: {value}")) }
///         Button {
///             text: "Increment",
///             on_click: move |_| count.update(|value| *value += 1),
///         }
///     }
/// }
/// ```
#[proc_macro]
pub fn ui(input: TokenStream) -> TokenStream {
  ui::expand(input.into())
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}
