//! Frame times for a scene of 1,000 elements (M04 "Done when" item 6). Run
//! with `cargo run --release -p dovetail-render --example scene_1000`.
//!
//! The scene is a 20 × 25 grid of 500 cells. Each cell is a box with a
//! background, a border, rounded corners and a text, so 1,000 elements plus
//! the grid's rows. A frame is: build the changed components, lay out,
//! paint into the canvas, and wait for the GPU to finish. It is drawn
//! headless, without copying to a window.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use dovetail_core::{
  Border, BuildContext, Callback, Color, Component, Edges, Element,
  ElementKind, Signal, Size, Style, Tree, View,
};
use dovetail_layout::Layout;
use dovetail_render::{Canvas, Gpu, Painter, font_measure};

const ROWS: usize = 25;
const COLUMNS: usize = 20;
const RUNS: usize = 15;
const WIDTH: f32 = 1600.0;
const HEIGHT: f32 = 1000.0;

/// Each cell's text and colour.
type Cells = Vec<(String, u32)>;

struct Grid;

impl Component for Grid {
  type Props = Callback<Signal<Cells>>;

  fn build(
    share: &Self::Props,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let cells = cx.use_state(|| {
      (0..ROWS * COLUMNS)
        .map(|i| (format!("cell {i}"), 0xdce0e8))
        .collect::<Cells>()
    });
    share.call(cells);
    let cells = cells.get();
    let rows = cells.chunks(COLUMNS).map(|row| {
      let row = row.iter().map(|(text, colour)| {
        Element::new(ElementKind::Box)
          .style(Style {
            width: Size::Fixed(76.0),
            padding: Edges::all(6.0),
            background: Color::hex(*colour),
            border: Border { width: 1.0, color: Color::hex(0x8c8fa1) },
            radius: 4.0,
            font_size: Some(12.0),
            ..Style::default()
          })
          .children([View::text(text.clone())])
          .into()
      });
      Element::new(ElementKind::Row)
        .style(Style { gap: 4.0, ..Style::default() })
        .children(row)
        .into()
    });
    Element::new(ElementKind::Column)
      .style(Style {
        gap: 4.0,
        padding: Edges::all(8.0),
        ..Style::default()
      })
      .children(rows)
      .into()
  }
}

struct Scene {
  gpu: Gpu,
  painter: Painter,
  canvas: Canvas,
  tree: Tree,
  layout: Layout,
  cells: Signal<Cells>,
}

struct Frame {
  time: Duration,
  laid_out: usize,
  boxes: usize,
  pixels: u64,
}

impl Scene {
  fn new(gpu: Gpu) -> Self {
    let painter = Painter::new(&gpu);
    let canvas = painter.canvas(&gpu, WIDTH as u32, HEIGHT as u32);
    let shared = Rc::new(RefCell::new(None));
    let slot = shared.clone();
    let mut tree = Tree::new();
    tree.mount(View::component::<Grid>(Callback::new(move |s| {
      *slot.borrow_mut() = Some(s)
    })));
    let cells = shared.borrow().expect("cells");
    Self {
      gpu,
      painter,
      canvas,
      tree,
      layout: Layout::new(),
      cells,
    }
  }

  fn frame(&mut self) -> Frame {
    let start = Instant::now();
    self.tree.update();
    let changes =
      self
        .layout
        .update(&self.tree, WIDTH, HEIGHT, &font_measure());
    let region = changes
      .damage_bounds()
      .or(Some(dovetail_layout::Rect::default()));
    let stats = self.painter.paint(
      &self.gpu,
      &mut self.canvas,
      &dovetail_render::PaintInput {
        boxes: self.layout.boxes(),
        region,
        scale: 1.0,
        background: Color::WHITE,
        focus_ring: None,
        caret_visible: true,
        colors: Default::default(),
        scrollbars: &[],
      },
    );
    self
      .gpu
      .device()
      .poll(wgpu_wait())
      .expect("wait for the GPU");
    Frame {
      time: start.elapsed(),
      laid_out: changes.stats.laid_out,
      boxes: changes.stats.boxes,
      pixels: stats.pixels(),
    }
  }
}

fn wgpu_wait() -> dovetail_render::PollType {
  dovetail_render::PollType::wait_indefinitely()
}

fn median(mut frames: Vec<Frame>) -> Frame {
  frames.sort_by_key(|f| f.time);
  frames.swap_remove(frames.len() / 2)
}

fn main() {
  let gpu = Gpu::headless().expect("a GPU adapter");
  let info = gpu.adapter_info();
  let mut scene = Scene::new(gpu);

  let first = median(
    (0..RUNS)
      .map(|_| {
        // A new canvas and layout: everything is laid out and painted.
        scene.canvas =
          scene
            .painter
            .canvas(&scene.gpu, WIDTH as u32, HEIGHT as u32);
        scene.layout = Layout::new();
        scene.frame()
      })
      .collect(),
  );
  let colour = median(
    (0..RUNS)
      .map(|i| {
        scene.cells.update(|cells| {
          cells[250].1 = if i % 2 == 0 { 0xf38ba8 } else { 0xa6e3a1 }
        });
        scene.frame()
      })
      .collect(),
  );
  let text = median(
    (0..RUNS)
      .map(|i| {
        scene
          .cells
          .update(|cells| cells[250].0 = format!("changed {i}"));
        scene.frame()
      })
      .collect(),
  );
  let idle = median((0..RUNS).map(|_| scene.frame()).collect());

  println!(
    "GPU: {} ({:?}), {} x {} pixels, median of {RUNS}",
    info.name, info.backend, WIDTH, HEIGHT
  );
  println!("| Frame | Time | Boxes laid out | Pixels painted |");
  println!("|---|---|---|---|");
  for (name, f) in [
    ("First frame (everything)", &first),
    ("One cell's colour changed", &colour),
    ("One cell's text changed", &text),
    ("Nothing changed", &idle),
  ] {
    println!(
      "| {name} | {:.2} ms | {} of {} | {} |",
      f.time.as_secs_f64() * 1e3,
      f.laid_out,
      f.boxes,
      f.pixels
    );
  }
}
