extern crate dovetail_core as dovetail;

use dovetail_macros::component;

#[component]
fn Label(#[prop(colour)] text: String) -> dovetail::View {
  dovetail::View::text(text)
}

fn main() {}
