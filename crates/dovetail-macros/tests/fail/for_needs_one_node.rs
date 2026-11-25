extern crate dovetail_core as dovetail;

use dovetail::View;
use dovetail_macros::{component, ui};

#[component]
fn Label(#[prop(into)] text: String, #[prop(default)] size: u32) -> View {
  ui! { { format!("{text} {size}") } }
}

fn main() { let items = [1, 2]; let _ = ui! { for i in items, key = i { "a" "b" } }; }
