//! Frame times with 10,000 shapes (M09 "Done when" item 5), headless. A
//! frame is: build what changed, lay out, paint (with the canvas's own
//! pass), and wait for the GPU to finish.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use dovetail::desktop::{KeyEvent, Modifiers, NamedKey};
use dovetail::layout::{Content, Layout, Rect};
use dovetail::prelude::*;
use dovetail::render::{Gpu, PaintInput, Painter, PollType, font_measure};
use dovetail::{Role, WheelEvent};

use crate::board::Board;

const WIDTH: f32 = 1600.0;
const HEIGHT: f32 = 1000.0;
const RUNS: usize = 15;

struct Bench {
  gpu: Gpu,
  painter: Painter,
  target: dovetail::render::Canvas,
  tree: Tree,
  layout: Layout,
}

impl Bench {
  /// One frame: build, lay out, paint what changed, and wait for the GPU.
  fn frame(&mut self) -> Duration {
    let start = Instant::now();
    self.tree.update();
    let changes =
      self
        .layout
        .update(&self.tree, WIDTH, HEIGHT, &font_measure());
    self.painter.paint(
      &self.gpu,
      &mut self.target,
      &PaintInput {
        region: changes.damage_bounds().or(Some(Rect::default())),
        ..PaintInput::new(self.layout.boxes(), 1.0)
      },
    );
    self
      .gpu
      .device()
      .poll(PollType::wait_indefinitely())
      .expect("wait for the GPU");
    start.elapsed()
  }

  /// The canvas element.
  fn canvas(&self) -> std::rc::Rc<dovetail::Element> {
    self
      .layout
      .boxes()
      .iter()
      .find_map(|b| match &b.content {
        Content::Element(e) if e.role == Role::Canvas => Some(e.clone()),
        _ => None,
      })
      .expect("the canvas")
  }

  fn wheel(
    &self,
    dy: f32,
  ) {
    let on_wheel = self
      .canvas()
      .on_wheel
      .clone()
      .expect("a wheel handler");
    on_wheel.call(WheelEvent {
      x: WIDTH / 2.0,
      y: HEIGHT / 2.0,
      dx: 0.0,
      dy,
      steps: true,
      modifiers: Modifiers::default(),
    });
  }

  fn key(
    &self,
    event: KeyEvent,
  ) {
    let on_key = self
      .canvas()
      .on_key
      .clone()
      .expect("a key handler");
    on_key.handle(&event);
  }
}

fn median(mut times: Vec<Duration>) -> Duration {
  times.sort();
  times[times.len() / 2]
}

fn ms(d: Duration) -> String {
  format!("{:.2} ms", d.as_secs_f64() * 1000.0)
}

pub fn run() -> ExitCode {
  let Ok(gpu) = Gpu::headless() else {
    eprintln!("gpu-canvas: no GPU adapter");
    return ExitCode::FAILURE;
  };
  let painter = Painter::new(&gpu);
  let target = painter.canvas(&gpu, WIDTH as u32, HEIGHT as u32);
  let mut tree = Tree::new();
  tree.mount(ui! { Board { large: true, width: WIDTH, height: HEIGHT } });
  let mut bench = Bench {
    gpu,
    painter,
    target,
    tree,
    layout: Layout::new(),
  };

  // The first frame makes the pipeline and the shapes' buffer.
  let first = bench.frame();
  // Warm up, then measure.
  for _ in 0..5 {
    bench.wheel(-12.0);
    bench.frame();
  }
  let zoom = median(
    (0..RUNS)
      .map(|i| {
        bench.wheel(if i % 2 == 0 { -48.0 } else { 48.0 });
        bench.frame()
      })
      .collect(),
  );
  let all = {
    let mut m = Modifiers::default();
    if cfg!(target_os = "macos") {
      m.meta = true;
    } else {
      m.control = true;
    }
    let mut e = KeyEvent::character('a').with(m);
    e.text = None;
    e
  };
  let select = median(
    (0..RUNS)
      .map(|i| {
        if i % 2 == 0 {
          bench.key(all.clone());
        } else {
          bench.key(KeyEvent::named(NamedKey::Escape));
        }
        bench.frame()
      })
      .collect(),
  );
  let idle = median((0..RUNS).map(|_| bench.frame()).collect());

  let info = bench.gpu.adapter_info();
  println!(
    "GPU: {} ({:?}), canvas {} x {} pixels, 10,000 shapes, median of {RUNS}",
    info.name, info.backend, WIDTH, HEIGHT
  );
  println!("| Frame | Time |");
  println!("|---|---|");
  println!(
    "| First frame (pipeline, buffer and everything) | {} |",
    ms(first)
  );
  println!(
    "| Zoom by one wheel notch (canvas redrawn) | {} |",
    ms(zoom)
  );
  println!(
    "| Select all or none (shapes' buffer rebuilt) | {} |",
    ms(select)
  );
  println!("| Nothing changed | {} |", ms(idle));
  ExitCode::SUCCESS
}
