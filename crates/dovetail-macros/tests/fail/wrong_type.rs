extern crate dovetail_core as dovetail;

use dovetail::View;
use dovetail_macros::{component, ui};

#[component]
fn Label(#[prop(into)] text: String, #[prop(default)] size: u32) -> View {
  ui! { { format!("{text} {size}") } }
}

fn main() { let _ = ui! { Label { text: "a", size: "large" } }; }
