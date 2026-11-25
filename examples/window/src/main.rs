//! M01 example: opens a window and clears it to a colour with wgpu.
//!
//! Run it with `cargo run -p example-window`.
//!
//! | Key | What it does |
//! |---|---|
//! | `N` | Opens another window. |
//! | `C` | Changes this window's colour. |
//! | `L` | Destroys the GPU device on purpose, to test recovery. |
//! | `Esc` | Closes this window. The example stops when the last one closes. |
//!
//! Each window prints its size and display scale whenever they change.
//!
//! `cargo run -p example-window -- --self-test` runs the automatic M01
//! checks instead and exits with a failure code if any fails.

mod self_test;

use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::atomic::Ordering;

use dovetail_desktop::{
  Application, Context, Frame, KeyCode, KeyEvent, NamedKey, WindowId,
  WindowOptions, run,
};
use dovetail_render::Color;

const COLOURS: [Color; 4] = [
  Color::hex(0x1e66f5),
  Color::hex(0x40a02b),
  Color::hex(0xdf8e1d),
  Color::hex(0x8839ef),
];

#[derive(Default)]
struct Example {
  colour_index: HashMap<WindowId, usize>,
  opened: usize,
}

impl Example {
  fn open(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    self.opened += 1;
    let title = format!("Dovetail window {}", self.opened);
    match cx.open_window(WindowOptions::new(title)) {
      Ok(id) => {
        self
          .colour_index
          .insert(id, (self.opened - 1) % COLOURS.len());
      },
      Err(e) => eprintln!("window: {e}"),
    }
  }
}

impl Application for Example {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    self.open(cx);
    if let Some(info) = cx.gpu_info() {
      println!("GPU: {} ({:?})", info.name, info.backend);
    }
  }

  fn draw(
    &mut self,
    _cx: &mut Context<'_>,
    window: WindowId,
    frame: &mut Frame,
  ) {
    let index = self
      .colour_index
      .get(&window)
      .copied()
      .unwrap_or(0);
    frame.clear(COLOURS[index]);
  }

  fn key_pressed(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
    key: &KeyEvent,
  ) {
    match &key.key {
      KeyCode::Character(c) if c.eq_ignore_ascii_case("n") => self.open(cx),
      KeyCode::Character(c) if c.eq_ignore_ascii_case("c") => {
        let index = self.colour_index.entry(window).or_default();
        *index = (*index + 1) % COLOURS.len();
        cx.request_redraw(window);
      },
      KeyCode::Character(c) if c.eq_ignore_ascii_case("l") => {
        println!("Simulating GPU device loss...");
        cx.simulate_device_loss();
      },
      KeyCode::Named(NamedKey::Escape) => self.close_requested(cx, window),
      _ => {},
    }
  }

  fn resized(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
    info: dovetail_desktop::WindowInfo,
  ) {
    let logical = info.logical_size();
    println!(
      "{window:?}: {}x{} physical px, scale {}, {}x{} logical px, GPU generation {}",
      info.size.width,
      info.size.height,
      info.scale_factor,
      logical.width,
      logical.height,
      cx.gpu_generation(),
    );
  }

  fn close_requested(
    &mut self,
    cx: &mut Context<'_>,
    window: WindowId,
  ) {
    self.colour_index.remove(&window);
    cx.close_window(window);
  }
}

fn main() -> ExitCode {
  let result = if std::env::args().any(|a| a == "--self-test") {
    run(self_test::SelfTest::new())
  } else {
    run(Example::default())
  };
  match result {
    Ok(()) if !self_test::FAILED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("window: {e}");
      ExitCode::FAILURE
    },
  }
}
