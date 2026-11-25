//! M09 example: a small PCB-like scene in a canvas that draws with its own
//! wgpu pipeline, with pan, zoom and selection.
//!
//! - `cargo run -p example-gpu-canvas` opens it. Click and drag to select,
//!   drag with the right button to pan, and turn the wheel to zoom.
//! - `cargo run -p example-gpu-canvas -- --self-test` zooms at a pad and
//!   checks it stays under the pointer, pans, and selects with clicks and a
//!   box, by itself.
//! - `cargo run --release -p example-gpu-canvas -- --bench` measures frame
//!   times with 10,000 shapes, headless.

mod bench;
mod board;
mod render;
mod scene;
mod self_test;

use std::process::ExitCode;

use dovetail::prelude::*;
use dovetail::{Edges, Element, ElementKind, Style};

use crate::board::Board;

#[component]
fn App() -> View {
  let large = use_state(|| false);
  let board = if large.get() {
    ui! { Board { large: true } }.keyed("large")
  } else {
    ui! { Board {} }.keyed("small")
  };
  Element::new(ElementKind::Column)
    .style(Style {
      padding: Edges::all(16.0),
      gap: 10.0,
      ..Style::default()
    })
    .children([
      ui! {
        Toggle {
          on: large.get(),
          on_change: move |on| large.set(on),
          label: "10,000 shapes",
        }
      },
      board,
    ])
    .into()
}

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().collect();
  if args.iter().any(|a| a == "--self-test") {
    return self_test::run();
  }
  if args.iter().any(|a| a == "--bench") {
    return bench::run();
  }
  match dovetail::run("GPU canvas", ui! { App {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("gpu-canvas: {e}");
      ExitCode::FAILURE
    },
  }
}
