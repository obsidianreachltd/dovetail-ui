//! The self-test: docks panels on every side and as tabs by dragging their
//! tabs, gives up a drag with Escape, resizes a split, floats a panel by
//! dragging it out of the window and drags it back, moves between panels
//! with F6 and between tabs with Ctrl+Page Down, and reads the saved
//! layout back.
//!
//! Every pointer and key event is sent to the window by the program, not
//! through the system, so nothing reaches other windows.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dovetail::desktop::{
  Application, Context, Frame, KeyEvent, Modifiers, NamedKey, WindowId,
  WindowOptions,
};
use dovetail::docking::{Axis, DockLayout, Node, StackAt};
use dovetail::layout::{Content, LayoutBox, Rect};
use dovetail::prelude::*;
use dovetail::{PointerKind, Role};

use crate::{DOCK, default_layout};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
  Start,
  DockLeft,
  DockRightEdge,
  DockBottom,
  DockTop,
  DockAsTab,
  EscapeCancels,
  Resize,
  Float,
  DragBack,
  F6(usize),
  NextTab,
  Saved,
}

impl Step {
  fn says(self) -> String {
    match self {
      Step::Start => "the default layout is shown".into(),
      Step::DockLeft => {
        "Inspector dragged to the left side of Board docks beside it".into()
      },
      Step::DockRightEdge => {
        "Console dragged to the right edge docks along it".into()
      },
      Step::DockBottom => {
        "Files dragged to the bottom of Board docks below it".into()
      },
      Step::DockTop => {
        "Layers dragged to the top of Console docks above it".into()
      },
      Step::DockAsTab => {
        "Inspector dragged onto Board's tab joins its stack".into()
      },
      Step::EscapeCancels => {
        "Escape gives up a drag and changes nothing".into()
      },
      Step::Resize => "dragging a splitter resizes the split".into(),
      Step::Float => {
        "Console dragged out of the window floats in its own window".into()
      },
      Step::DragBack => {
        "Console dragged back beside Board docks again, and its window closes"
          .into()
      },
      Step::F6(n) => {
        format!("F6 number {} moves focus to the next panel", n + 1)
      },
      Step::NextTab => {
        "Ctrl+Page Down shows the next tab and focuses it".into()
      },
      Step::Saved => "the saved file reads back as the layout shown".into(),
    }
  }
}

struct Test {
  file: PathBuf,
  main: Option<WindowId>,
  steps: Vec<Step>,
  at: usize,
  acted: bool,
  /// The layout before an action that should change nothing, or the
  /// first share before a resize.
  before: Option<DockLayout>,
  /// Where focus went after each F6.
  visited: Vec<String>,
}

fn element(b: &LayoutBox) -> Option<&dovetail::Element> {
  match &b.content {
    Content::Element(e) => Some(e),
    _ => None,
  }
}

fn centre(r: Rect) -> (f32, f32) {
  (r.x + r.width / 2.0, r.y + r.height / 2.0)
}

/// The tab showing `title`.
fn tab(
  boxes: &[LayoutBox],
  title: &str,
) -> Option<Rect> {
  boxes.iter().find_map(|b| {
    let e = element(b)?;
    let first = boxes.get(*b.children.first()?)?;
    let text =
      matches!(&first.content, Content::Text { text, .. } if &**text == title);
    (e.role == Role::Tab && text).then_some(b.rect)
  })
}

/// The content of the panel called `title`.
fn panel(
  boxes: &[LayoutBox],
  title: &str,
) -> Option<Rect> {
  boxes.iter().find_map(|b| {
    let e = element(b)?;
    (e.role == Role::TabPanel && e.label.as_deref() == Some(title))
      .then_some(b.rect)
  })
}

/// The splitter that runs the whole height of the window, the first one.
fn tallest_splitter(boxes: &[LayoutBox]) -> Option<Rect> {
  boxes
    .iter()
    .filter(|b| element(b).is_some_and(|e| e.role == Role::Splitter))
    .map(|b| b.rect)
    .fold(None, |best: Option<Rect>, r| match best {
      Some(b) if b.height >= r.height => Some(b),
      _ => Some(r),
    })
}

/// True when `a` and `b` are next to each other, `a` first, in a split
/// that goes `axis`.
fn neighbours(
  layout: &DockLayout,
  a: &str,
  b: &str,
  axis: Axis,
) -> bool {
  let (Some((StackAt::Docked(pa), _)), Some((StackAt::Docked(pb), _))) =
    (layout.find(a), layout.find(b))
  else {
    return false;
  };
  let (Some((ia, parent_a)), Some((ib, parent_b))) =
    (pa.split_last(), pb.split_last())
  else {
    return false;
  };
  if parent_a != parent_b || ia + 1 != *ib {
    return false;
  }
  matches!(
    layout.root.as_ref().and_then(|r| r.at(parent_a)),
    Some(Node::Split { axis: s, .. }) if *s == axis
  )
}

fn drag(
  cx: &mut Context<'_>,
  window: WindowId,
  from: (f32, f32),
  to: (f32, f32),
) {
  let none = Modifiers::default();
  cx.pointer(window, PointerKind::Down, from.0, from.1, none);
  cx.pointer(window, PointerKind::Move, from.0 + 10.0, from.1 + 2.0, none);
  for t in [0.5, 0.9, 1.0] {
    let p = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
    cx.pointer(window, PointerKind::Move, p.0, p.1, none);
  }
  cx.pointer(window, PointerKind::Up, to.0, to.1, none);
}

/// Says where focus is in `window`, to tell panels apart.
fn focus_name(
  cx: &Context<'_>,
  window: WindowId,
) -> Option<String> {
  let e = cx.focused_element(window)?;
  Some(format!("{:?} {:?}", e.role, e.label))
}

fn root_share(layout: &DockLayout) -> Option<f32> {
  match &layout.root {
    Some(Node::Split { parts, .. }) => Some(parts[0].0),
    _ => None,
  }
}

impl Test {
  /// Does step `step`. False when the window is not ready for it yet.
  fn act(
    &mut self,
    cx: &mut Context<'_>,
    main: WindowId,
    step: Step,
  ) -> bool {
    let Some(dock) = DOCK.get() else {
      return false;
    };
    let boxes = cx.ui_boxes(main).to_vec();
    let size = cx
      .window_info(main)
      .map(|i| i.logical_size())
      .map(|s| (s.width as f32, s.height as f32))
      .unwrap_or((800.0, 600.0));
    let zone = |title: &str, fx: f32, fy: f32| {
      panel(&boxes, title).map(|r| (r.x + r.width * fx, r.y + r.height * fy))
    };
    let from = |title: &str| tab(&boxes, title).map(centre);
    match step {
      Step::Start => true,
      Step::DockLeft => match (from("Inspector"), zone("Board", 0.08, 0.5)) {
        (Some(a), Some(b)) => {
          drag(cx, main, a, b);
          true
        },
        _ => false,
      },
      Step::DockRightEdge => match from("Console") {
        Some(a) => {
          drag(cx, main, a, (size.0 - 3.0, size.1 / 2.0));
          true
        },
        None => false,
      },
      Step::DockBottom => match (from("Files"), zone("Board", 0.5, 0.92)) {
        (Some(a), Some(b)) => {
          drag(cx, main, a, b);
          true
        },
        _ => false,
      },
      Step::DockTop => match (from("Layers"), zone("Console", 0.5, 0.08)) {
        (Some(a), Some(b)) => {
          drag(cx, main, a, b);
          true
        },
        _ => false,
      },
      Step::DockAsTab => match (from("Inspector"), tab(&boxes, "Board")) {
        (Some(a), Some(b)) => {
          drag(cx, main, a, (b.x + b.width * 0.75, b.y + b.height / 2.0));
          true
        },
        _ => false,
      },
      Step::EscapeCancels => match (from("Files"), zone("Inspector", 0.5, 0.5))
      {
        (Some(a), Some(b)) => {
          self.before = Some(dock.layout());
          let none = Modifiers::default();
          cx.pointer(main, PointerKind::Down, a.0, a.1, none);
          cx.pointer(main, PointerKind::Move, a.0 + 10.0, a.1, none);
          cx.pointer(main, PointerKind::Move, b.0, b.1, none);
          if !cx.dragging() {
            println!("FAIL  the tab did not start a drag");
          }
          cx.key(main, &KeyEvent::named(NamedKey::Escape));
          cx.pointer(main, PointerKind::Up, b.0, b.1, none);
          // Nothing changes, so ask for a frame to check in.
          cx.request_redraw(main);
          true
        },
        _ => false,
      },
      Step::Resize => match tallest_splitter(&boxes) {
        Some(r) => {
          self.before = Some(dock.layout());
          let (x, y) = centre(r);
          let none = Modifiers::default();
          cx.pointer(main, PointerKind::Down, x, y, none);
          cx.pointer(main, PointerKind::Move, x + 25.0, y, none);
          cx.pointer(main, PointerKind::Move, x + 50.0, y, none);
          cx.pointer(main, PointerKind::Up, x + 50.0, y, none);
          true
        },
        None => false,
      },
      Step::Float => match from("Console") {
        Some(a) => {
          drag(cx, main, a, (size.0 + 220.0, size.1 / 3.0));
          true
        },
        None => false,
      },
      Step::DragBack => {
        let Some(float) = cx.windows().into_iter().find(|w| *w != main) else {
          return false;
        };
        let float_boxes = cx.ui_boxes(float).to_vec();
        // Inspector now shares Board's stack, and is the tab shown.
        let (Some(a), Some(target)) =
          (tab(&float_boxes, "Console"), panel(&boxes, "Inspector"))
        else {
          return false;
        };
        let a = centre(a);
        match (cx.window_origin(main), cx.window_origin(float)) {
          (Some(m), Some(f)) => {
            // Near the left side of Board's stack, in the floating
            // window's coordinates.
            let (tx, ty) = (
              target.x + target.width * 0.1,
              target.y + target.height / 2.0,
            );
            drag(cx, float, a, (m.0 + tx - f.0, m.1 + ty - f.1));
          },
          // The system does not say where windows are: use the button.
          _ => {
            println!(
              "NOTE  window places are not known here; using the Dock button"
            );
            dock.dock_floating(dock.layout().floating[0].id);
          },
        }
        true
      },
      Step::F6(_) => {
        cx.key(main, &KeyEvent::named(NamedKey::F6));
        true
      },
      Step::NextTab => {
        if self.before.is_none() {
          // First put focus in Inspector, which shares Board's stack.
          dock.focus_panel("inspector");
          self.before = Some(dock.layout());
          return false;
        }
        if cx
          .focused_element(main)
          .and_then(|e| e.label.clone())
          .as_deref()
          != Some("Name")
        {
          return false;
        }
        let mut e = KeyEvent::named(NamedKey::PageDown);
        e.modifiers = Modifiers { control: true, ..Modifiers::default() };
        cx.key(main, &e);
        true
      },
      Step::Saved => {
        cx.request_redraw(main);
        true
      },
    }
  }

  /// True once step `step` has done what it should.
  fn check(
    &mut self,
    cx: &mut Context<'_>,
    main: WindowId,
    step: Step,
  ) -> bool {
    let Some(dock) = DOCK.get() else {
      return false;
    };
    let layout = dock.layout();
    match step {
      Step::Start => {
        layout == default_layout()
          && tab(cx.ui_boxes(main), "Inspector").is_some()
      },
      Step::DockLeft => neighbours(&layout, "inspector", "board", Axis::Row),
      Step::DockRightEdge => match &layout.root {
        Some(Node::Split { axis: Axis::Row, parts }) => matches!(
          parts.last(),
          Some((_, Node::Tabs(t))) if t.panels == ["console"]
        ),
        _ => false,
      },
      Step::DockBottom => neighbours(&layout, "board", "files", Axis::Column),
      Step::DockTop => neighbours(&layout, "layers", "console", Axis::Column),
      Step::DockAsTab => {
        matches!((layout.find("board"), layout.find("inspector")), (Some((a, _)), Some((b, _))) if a == b)
      },
      Step::EscapeCancels => {
        !cx.dragging() && Some(&layout) == self.before.as_ref()
      },
      Step::Resize => {
        let before = self.before.as_ref().and_then(root_share);
        let after = root_share(&layout);
        matches!((before, after), (Some(b), Some(a)) if a > b + 0.02)
      },
      Step::Float => {
        let floats = layout
          .floating
          .iter()
          .any(|f| f.tabs.panels == ["console"]);
        let window = cx.windows().into_iter().find(|w| *w != main);
        floats
          && window.is_some_and(|w| {
            cx.window_title(w).as_deref() == Some("Console")
              && tab(cx.ui_boxes(w), "Console").is_some()
          })
      },
      Step::DragBack => {
        layout.floating.is_empty()
          && neighbours(&layout, "console", "inspector", Axis::Row)
          && cx.window_count() == 1
      },
      Step::F6(n) => {
        let Some(name) = focus_name(cx, main) else {
          return false;
        };
        let stacks = layout.stacks().len();
        if n < stacks {
          if self.visited.contains(&name) {
            println!(
              "FAIL  F6 came back to {name} too soon: {:?}",
              self.visited
            );
            cx.exit();
            return false;
          }
          self.visited.push(name);
          true
        } else {
          // One more goes round to the first again.
          let first = self.visited.first().cloned();
          if Some(&name) != first.as_ref() {
            println!("FAIL  F6 went to {name}, not back to {first:?}");
            cx.exit();
            return false;
          }
          true
        }
      },
      Step::NextTab => {
        let shown = layout.find("board").and_then(|(at, _)| {
          layout
            .tabs(&at)
            .map(|t| t.active_panel().to_string())
        });
        shown.as_deref() == Some("board")
          && cx
            .focused_element(main)
            .and_then(|e| e.label.clone())
            .as_deref()
            == Some("Board")
      },
      Step::Saved => {
        let Ok(text) = std::fs::read_to_string(&self.file) else {
          return false;
        };
        DockLayout::from_text(&text)
          .is_ok_and(|saved| saved.to_text() == layout.to_text())
      },
    }
  }
}

impl Application for Test {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let _ = std::fs::remove_file(&self.file);
    let file = Some(self.file.clone());
    let options = WindowOptions::new("Docking self-test")
      .with_size(dovetail::desktop::LogicalSize::new(900.0, 640.0));
    match cx.open_ui_window(options, ui! { crate::App { file: file } }) {
      Ok(id) => self.main = Some(id),
      Err(e) => {
        println!("FAIL  could not open the window: {e}");
        cx.exit();
      },
    }
    cx.wake_after(Duration::from_secs(30));
  }

  fn draw(
    &mut self,
    _: &mut Context<'_>,
    _: WindowId,
    _: &mut Frame,
  ) {
  }

  fn frame_presented(
    &mut self,
    cx: &mut Context<'_>,
    _: WindowId,
  ) {
    let Some(main) = self.main else {
      return;
    };
    let Some(&step) = self.steps.get(self.at) else {
      return;
    };
    if self.acted {
      if !self.check(cx, main, step) {
        return;
      }
      println!("PASS  {}", step.says());
      self.at += 1;
      self.acted = false;
      self.before = None;
      let Some(&next) = self.steps.get(self.at) else {
        PASSED.store(true, Ordering::Relaxed);
        println!(
          "PASS  docking by drag and drop, resizing, floating, keys and saving work"
        );
        let _ = std::fs::remove_file(&self.file);
        cx.close_window(main);
        return;
      };
      let _ = next;
    }
    let step = self.steps[self.at];
    // Either way there is a frame to look at next: to check the step, or
    // to try it again when it was not ready.
    self.acted = self.act(cx, main, step);
    cx.request_redraw(main);
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let waiting = self
      .steps
      .get(self.at)
      .map_or_else(String::new, |s| s.says());
    println!("FAIL  timed out waiting for: {waiting}");
    if let Some(dock) = DOCK.get() {
      println!("      the layout is:\n{}", dock.layout().to_text());
    }
    cx.exit();
  }
}

static PASSED: AtomicBool = AtomicBool::new(false);

pub fn run() -> ExitCode {
  let file = std::env::temp_dir().join(format!(
    "dovetail-docking-self-test-{}.txt",
    std::process::id()
  ));
  let mut steps = vec![
    Step::Start,
    Step::DockLeft,
    Step::DockRightEdge,
    Step::DockBottom,
    Step::DockTop,
    Step::DockAsTab,
    Step::EscapeCancels,
    Step::Resize,
    Step::Float,
    Step::DragBack,
  ];
  // The layout then has four stacks: one F6 for each, and one more to go
  // round to the first again.
  steps.extend((0..5).map(Step::F6));
  steps.extend([Step::NextTab, Step::Saved]);
  let test = Test {
    file,
    main: None,
    steps,
    at: 0,
    acted: false,
    before: None,
    visited: Vec::new(),
  };
  match dovetail::desktop::run(test) {
    Ok(()) if PASSED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("docking: {e}");
      ExitCode::FAILURE
    },
  }
}
