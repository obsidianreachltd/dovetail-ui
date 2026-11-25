//! Build and scroll times for a list, a tree and a table of 100,000 rows
//! each (M16 "Done when" items 1 and 6), headless.
//!
//! A frame is what the window does: build what changed, lay out, tell the
//! views where they are scrolled to, build and lay out again while that
//! makes new rows, paint what changed, and wait for the GPU to finish.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use dovetail::data::{
  Column as TableColumn, DataList, DataTable, DataTree, Expansion, RowHeight,
  RowState, Selection, SelectionMode,
};
use dovetail::layout::{BoxKey, Content, Layout, Rect};
use dovetail::prelude::*;
use dovetail::render::{Gpu, PaintInput, Painter, PollType, font_measure};
use dovetail::{Role, Viewport};

use crate::{
  LAYERS, PARTS, SHAPES, base_part, log_entry, node_name, shape_key,
};

const WIDTH: f32 = 900.0;
const HEIGHT: f32 = 700.0;
const RUNS: usize = 31;

#[component]
fn BenchList(measured: bool) -> View {
  let selection = use_state(|| Selection::new(SelectionMode::Multiple)).get();
  ui! {
    DataList {
      count: PARTS,
      row: |i: usize, _: &RowState| View::text(log_entry(i)),
      selection: selection,
      row_height: if measured {
        RowHeight::Measured { estimate: 26.0 }
      } else {
        RowHeight::Theme
      },
      label: "List",
    }
  }
}

#[component]
fn BenchTree() -> View {
  let selection = use_state(|| Selection::new(SelectionMode::Single)).get();
  // Every layer open: 100,100 rows.
  let open = use_state(|| {
    let e = Expansion::new();
    for layer in 0..LAYERS {
      e.set(Key::from(layer), true);
    }
    e
  })
  .get();
  ui! {
    DataTree {
      nodes: |node: Option<&Key>| match node.and_then(Key::number) {
        None => (0..LAYERS).map(Key::from).collect(),
        Some(layer) => (0..SHAPES).map(|i| shape_key(layer, i)).collect(),
      },
      has_children: |k: &Key| k.number().is_some_and(|n| n < LAYERS),
      row: |k: &Key, _: &RowState| View::text(node_name(k)),
      selection: selection,
      expansion: Some(open),
      label: "Tree",
    }
  }
}

#[component]
fn BenchTable() -> View {
  let selection = use_state(|| Selection::new(SelectionMode::Multiple)).get();
  ui! {
    DataTable {
      columns: vec![
        TableColumn::new("Name", 160.0),
        TableColumn::new("Value", 120.0).at_end(),
        TableColumn::new("Footprint", 160.0),
        TableColumn::new("Side", 120.0),
      ],
      count: PARTS,
      cell: |row: usize, column: usize, _: &RowState| {
        let p = base_part(row as u32);
        View::text(match column {
          0 => p.name,
          1 => format!("{:.1}", p.value),
          2 => crate::FOOTPRINTS[p.footprint].to_string(),
          _ => crate::SIDES[p.side].to_string(),
        })
      },
      selection: selection,
      label: "Table",
    }
  }
}

struct Bench {
  gpu: Gpu,
  painter: Painter,
  target: dovetail::render::Canvas,
  tree: Tree,
  layout: Layout,
  told: Vec<(BoxKey, Viewport)>,
  sizes: Vec<(BoxKey, (f32, f32))>,
  /// How many times the last frame laid out.
  passes: usize,
}

impl Bench {
  fn new(
    gpu: Gpu,
    painter: Painter,
    root: View,
  ) -> Bench {
    let target = painter.canvas(&gpu, WIDTH as u32, HEIGHT as u32);
    let mut tree = Tree::new();
    tree.mount(root);
    Bench {
      gpu,
      painter,
      target,
      tree,
      layout: Layout::new(),
      told: Vec::new(),
      sizes: Vec::new(),
      passes: 0,
    }
  }

  /// Tells views where they are scrolled to and how big rows are, as the
  /// window does after layout.
  fn report(&mut self) {
    let mut calls: Vec<Box<dyn Fn()>> = Vec::new();
    let (mut told, mut sizes) = (Vec::new(), Vec::new());
    for b in self.layout.boxes() {
      let Content::Element(e) = &b.content else {
        continue;
      };
      if let (Some(f), Some(v)) = (&e.on_scroll, self.layout.viewport(&b.key)) {
        if !self.told.contains(&(b.key.clone(), v)) {
          let f = f.clone();
          calls.push(Box::new(move || f.call(v)));
        }
        told.push((b.key.clone(), v));
      }
      if let Some(f) = &e.on_size {
        let size = (b.rect.width, b.rect.height);
        if !self.sizes.contains(&(b.key.clone(), size)) {
          let f = f.clone();
          calls.push(Box::new(move || f.call(size)));
        }
        sizes.push((b.key.clone(), size));
      }
    }
    self.told = told;
    self.sizes = sizes;
    for call in calls {
      call();
    }
  }

  fn frame(&mut self) -> Duration {
    let start = Instant::now();
    let mut damage: Option<Rect> = None;
    self.passes = 0;
    for pass in 0..4 {
      if pass > 0 && !self.tree.needs_update() {
        break;
      }
      self.tree.update();
      let changes =
        self
          .layout
          .update(&self.tree, WIDTH, HEIGHT, &font_measure());
      if let Some(d) = changes.damage_bounds() {
        damage = Some(damage.map_or(d, |old| old.union(&d)));
      }
      self.report();
      self.passes += 1;
    }
    self.painter.paint(
      &self.gpu,
      &mut self.target,
      &PaintInput {
        region: damage.or(Some(Rect::default())),
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

  /// The scrolling body of the view.
  fn body(&self) -> Rect {
    self
      .layout
      .boxes()
      .iter()
      .find_map(|b| match &b.content {
        Content::Element(e)
          if e.style.scroll && e.style.scrollbars && e.focusable =>
        {
          Some(b.rect)
        },
        _ => None,
      })
      .expect("a scrolling view")
  }

  fn scroll(
    &mut self,
    dy: f32,
  ) {
    let r = self.body();
    self
      .layout
      .scroll_by(r.x + 10.0, r.y + 10.0, 0.0, dy);
  }

  fn rows(&self) -> usize {
    self
      .layout
      .boxes()
      .iter()
      .filter(|b| {
        matches!(&b.content, Content::Element(e)
          if matches!(e.role, Role::ListBoxOption | Role::TreeItem | Role::Row))
      })
      .count()
  }
}

fn median(mut times: Vec<Duration>) -> Duration {
  times.sort();
  times[times.len() / 2]
}

fn ms(d: Duration) -> String {
  format!("{:.2} ms", d.as_secs_f64() * 1000.0)
}

/// Measures one view: the first frame, scrolling a page at a time, a
/// small scroll, and a long jump.
fn measure(
  name: &str,
  root: View,
) -> String {
  let gpu = Gpu::headless().expect("a GPU adapter");
  let painter = Painter::new(&gpu);
  let mut bench = Bench::new(gpu, painter, root);
  let first = bench.frame();
  let rows = bench.rows();
  for _ in 0..5 {
    bench.scroll(40.0);
    bench.frame();
  }
  let page = median(
    (0..RUNS)
      .map(|_| {
        bench.scroll(HEIGHT - 60.0);
        bench.frame()
      })
      .collect(),
  );
  let passes = bench.passes;
  let small = median(
    (0..RUNS)
      .map(|i| {
        bench.scroll(if i % 2 == 0 { 40.0 } else { -40.0 });
        bench.frame()
      })
      .collect(),
  );
  let jump = median(
    (0..RUNS)
      .map(|i| {
        bench.scroll(if i % 2 == 0 { 1.0e6 } else { -1.0e6 });
        bench.frame()
      })
      .collect(),
  );
  format!(
    "| {name} | {} | {} | {} | {} | {rows} | {passes} |",
    ms(first),
    ms(page),
    ms(small),
    ms(jump)
  )
}

pub fn run() -> ExitCode {
  let Ok(gpu) = Gpu::headless() else {
    eprintln!("data: no GPU adapter");
    return ExitCode::FAILURE;
  };
  let lines = [
    measure("List, 100,000 rows", ui! { BenchList { measured: false } }),
    measure(
      "List, 100,000 measured rows",
      ui! { BenchList { measured: true } },
    ),
    measure("Tree, 100,100 open rows", ui! { BenchTree {} }),
    measure("Table, 100,000 rows, 4 columns", ui! { BenchTable {} }),
  ];
  let info = gpu.adapter_info();
  println!(
    "GPU: {} ({:?}), view {WIDTH} x {HEIGHT} pixels, median of {RUNS}",
    info.name, info.backend
  );
  println!(
    "| View | First frame | Scroll a page | Scroll 40 pixels | Jump 1,000,000 pixels | Rows made | Layouts in a page frame |"
  );
  println!("|---|---|---|---|---|---|---|");
  for line in lines {
    println!("{line}");
  }
  ExitCode::SUCCESS
}
