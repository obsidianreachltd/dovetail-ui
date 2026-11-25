//! The self-test: uses each view in a real window, with the pointer and
//! with the keyboard, through the window's own test functions (no key
//! presses reach the system), and checks after each step:
//!
//! - the table: clicks with Shift, Ctrl+A, End and Home, sorting by a
//!   header, and resizing a column by dragging its edge;
//! - the property grid: a check box and a number edited, and each edit
//!   undone;
//! - dragging parts from the table to the Picked list;
//! - the tree: a click, Right to open and move in, Left to move out and
//!   close;
//! - the log: rows of different heights, scrolled with the wheel;
//! - the dark theme's row colours.

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dovetail::desktop::{
  Application, Context, Frame, LogicalSize, WindowId, WindowOptions,
};
use dovetail::layout::{Content, LayoutBox, Rect};
use dovetail::prelude::*;
use dovetail::theme::{ThemeData, window_style};
use dovetail::{KeyCode, KeyEvent, Modifiers, NamedKey, PointerKind, Role};

use crate::{PARTS, STATE, State};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
  Light,
  TableClick,
  TableShift,
  TableAll,
  TableEnd,
  TableHome,
  SortValue,
  SortValueDown,
  Resize,
  InspectorCheck,
  InspectorUndo,
  NumberUp,
  NumberUpAgain,
  NumberCommit,
  NumberUndo,
  DragToPicked,
  TreeClick,
  TreeOpen,
  TreeInto,
  TreeOut,
  TreeClose,
  LogScroll,
  Dark,
}

const STEPS: [Step; 23] = [
  Step::Light,
  Step::TableClick,
  Step::TableShift,
  Step::TableAll,
  Step::TableEnd,
  Step::TableHome,
  Step::SortValue,
  Step::SortValueDown,
  Step::Resize,
  Step::InspectorCheck,
  Step::InspectorUndo,
  Step::NumberUp,
  Step::NumberUpAgain,
  Step::NumberCommit,
  Step::NumberUndo,
  Step::DragToPicked,
  Step::TreeClick,
  Step::TreeOpen,
  Step::TreeInto,
  Step::TreeOut,
  Step::TreeClose,
  Step::LogScroll,
  Step::Dark,
];

impl Step {
  fn says(self) -> &'static str {
    match self {
      Step::Light => {
        "each view shows, making only the rows in sight, in light colours"
      },
      Step::TableClick => {
        "a click chooses a part and shows it in the inspector"
      },
      Step::TableShift => "Shift+Down adds the next two parts",
      Step::TableAll => "Ctrl+A selects all 100,000 parts",
      Step::TableEnd => "End moves to the last part and scrolls to it",
      Step::TableHome => "Home moves back to the first part",
      Step::SortValue => "a press on the Value header sorts by value",
      Step::SortValueDown => "a second press sorts the other way",
      Step::Resize => "dragging the Name header's edge widens the column",
      Step::InspectorCheck => "a click on Locked in the inspector changes it",
      Step::InspectorUndo => "Ctrl+Z undoes that",
      Step::NumberUp => "the Increase button raises the value by a step",
      Step::NumberUpAgain => "and again",
      Step::NumberCommit => "leaving the field keeps both as one undo step",
      Step::NumberUndo => "the Undo button takes both back",
      Step::DragToPicked => "two parts dragged from the table drop in Picked",
      Step::TreeClick => "a click chooses a layer in the tree",
      Step::TreeOpen => "Right opens it, making its shapes",
      Step::TreeInto => "Right again moves to its first shape",
      Step::TreeOut => "Left moves back to the layer",
      Step::TreeClose => "Left again closes it",
      Step::LogScroll => {
        "the wheel scrolls the log, whose rows have different heights"
      },
      Step::Dark => "the views take the dark theme's colours",
    }
  }
}

fn element(b: &LayoutBox) -> Option<&dovetail::Element> {
  match &b.content {
    Content::Element(e) => Some(e),
    _ => None,
  }
}

fn text_of(
  boxes: &[LayoutBox],
  i: usize,
) -> String {
  let mut out = String::new();
  let mut stack = vec![i];
  while let Some(i) = stack.pop() {
    match &boxes[i].content {
      Content::Text { text, .. } => out.push_str(text),
      Content::Element(_) => stack.extend(boxes[i].children.iter().rev()),
      Content::Picture { .. } => {},
    }
  }
  out
}

/// The boxes inside box `i`, and it.
fn inside(
  boxes: &[LayoutBox],
  i: usize,
) -> Vec<usize> {
  let mut out = Vec::new();
  let mut stack = vec![i];
  while let Some(i) = stack.pop() {
    out.push(i);
    stack.extend(boxes[i].children.iter().copied());
  }
  out
}

/// The element with `role` whose label is `name` or whose text starts
/// with it.
fn find(
  boxes: &[LayoutBox],
  role: Role,
  name: &str,
) -> Option<usize> {
  (0..boxes.len()).find(|&i| {
    element(&boxes[i]).is_some_and(|e| {
      e.role == role
        && (e.label.as_deref() == Some(name)
          || text_of(boxes, i).starts_with(name))
    })
  })
}

/// The view labelled `view`, and its row with `role` at item `index`.
fn row(
  boxes: &[LayoutBox],
  view: (Role, &str),
  role: Role,
  index: usize,
) -> Option<usize> {
  let v = find(boxes, view.0, view.1)?;
  inside(boxes, v).into_iter().find(|&i| {
    element(&boxes[i]).is_some_and(|e| {
      e.role == role && e.item.is_some_and(|p| p.index == index)
    })
  })
}

/// A table row: the header is row 0, so part row `n` is item `n + 1`.
fn part_row(
  boxes: &[LayoutBox],
  n: usize,
) -> Option<usize> {
  row(boxes, (Role::Table, "Parts"), Role::Row, n + 1)
}

fn rows_made(
  boxes: &[LayoutBox],
  view: (Role, &str),
  role: Role,
) -> usize {
  find(boxes, view.0, view.1).map_or(0, |v| {
    inside(boxes, v)
      .into_iter()
      .filter(|&i| element(&boxes[i]).is_some_and(|e| e.role == role))
      .count()
  })
}

fn centre(r: Rect) -> (f32, f32) {
  (r.x + r.width / 2.0, r.y + r.height / 2.0)
}

fn state() -> State {
  STATE.get().expect("the app is built")
}

fn key(named: NamedKey) -> KeyEvent {
  KeyEvent::named(named)
}

fn command() -> Modifiers {
  if cfg!(target_os = "macos") {
    Modifiers { meta: true, ..Modifiers::default() }
  } else {
    Modifiers { control: true, ..Modifiers::default() }
  }
}

fn command_key(c: char) -> KeyEvent {
  KeyEvent {
    key: KeyCode::Character(c.to_string()),
    modifiers: command(),
    text: None,
    repeat: false,
  }
}

/// What the test remembers between steps.
#[derive(Default)]
struct Memory {
  /// The part being edited, and its value before.
  part: Option<u32>,
  value: f32,
  locked: bool,
  /// The log's first row's place before scrolling.
  log_top: usize,
}

struct Test {
  main: Option<WindowId>,
  at: usize,
  acted: bool,
  memory: Memory,
  last: String,
}

static PASSED: AtomicBool = AtomicBool::new(false);

impl Test {
  /// Does the step's action.
  fn act(
    &mut self,
    cx: &mut Context<'_>,
    main: WindowId,
    step: Step,
  ) -> Result<(), String> {
    let s = state();
    let boxes = cx.ui_boxes(main);
    let rect = |i: Option<usize>, what: &str| {
      i.map(|i| boxes[i].rect)
        .ok_or(format!("no {what} to use"))
    };
    let none = Modifiers::default();
    match step {
      Step::Light => s.dark.set(false),
      Step::TableClick => {
        let (x, y) = centre(rect(part_row(boxes, 2), "part row 2")?);
        cx.click(main, x, y);
      },
      Step::TableShift => {
        cx.key(
          main,
          &key(NamedKey::ArrowDown).with(Modifiers { shift: true, ..none }),
        );
        cx.key(
          main,
          &key(NamedKey::ArrowDown).with(Modifiers { shift: true, ..none }),
        );
      },
      Step::TableAll => {
        cx.key(main, &command_key('a'));
      },
      Step::TableEnd => {
        cx.key(main, &key(NamedKey::End));
      },
      Step::TableHome => {
        cx.key(main, &key(NamedKey::Home));
      },
      Step::SortValue | Step::SortValueDown => {
        let header = find(boxes, Role::ColumnHeader, "Value");
        let r = rect(header, "the Value header")?;
        cx.click(main, r.x + 20.0, r.y + r.height / 2.0);
      },
      Step::Resize => {
        let header = rect(find(boxes, Role::ColumnHeader, "Name"), "Name")?;
        // The edge is the last 6 pixels of the header.
        let (x, y) = (header.x + header.width - 3.0, header.y + 8.0);
        cx.pointer(main, PointerKind::Move, x, y, none);
        cx.pointer(main, PointerKind::Down, x, y, none);
        cx.pointer(main, PointerKind::Move, x + 20.0, y, none);
        cx.pointer(main, PointerKind::Move, x + 40.0, y, none);
        cx.pointer(main, PointerKind::Up, x + 40.0, y, none);
      },
      Step::InspectorCheck => {
        let part = s
          .parts
          .current()
          .and_then(|k| k.number())
          .ok_or("no part chosen")? as u32;
        self.memory.part = Some(part);
        self.memory.locked = s.part(part).locked;
        let (x, y) =
          centre(rect(find(boxes, Role::CheckBox, "Locked"), "Locked")?);
        cx.click(main, x, y);
      },
      Step::InspectorUndo => {
        cx.key(main, &command_key('z'));
      },
      Step::NumberUp => {
        let part = self.memory.part.ok_or("no part")?;
        self.memory.value = s.part(part).value;
        let up = rect(find(boxes, Role::Button, "Increase"), "Increase")?;
        let (x, y) = centre(up);
        cx.click(main, x, y);
      },
      Step::NumberUpAgain => {
        let up = rect(find(boxes, Role::Button, "Increase"), "Increase")?;
        let (x, y) = centre(up);
        cx.click(main, x, y);
      },
      Step::NumberCommit => {
        // A click on the Name field moves focus out of the number field.
        let (x, y) =
          centre(rect(find(boxes, Role::TextInput, "Name"), "Name")?);
        cx.click(main, x, y);
      },
      Step::NumberUndo => {
        let (x, y) = centre(rect(find(boxes, Role::Button, "Undo"), "Undo")?);
        cx.click(main, x, y);
      },
      Step::DragToPicked => {
        // Choose parts 4 and 5, then drag them.
        let (x, y) = centre(rect(part_row(boxes, 4), "part row 4")?);
        let (x5, y5) = centre(rect(part_row(boxes, 5), "part row 5")?);
        let picked = rect(find(boxes, Role::ListBox, "Picked"), "Picked")?;
        cx.click(main, x, y);
        // Shift+Down adds part 5. A second click on it would be a double
        // click.
        cx.key(
          main,
          &key(NamedKey::ArrowDown).with(Modifiers { shift: true, ..none }),
        );
        let (px, py) = centre(picked);
        cx.pointer(main, PointerKind::Down, x5, y5, none);
        cx.pointer(main, PointerKind::Move, x5 + 10.0, y5 + 10.0, none);
        cx.pointer(main, PointerKind::Move, px, py, none);
        cx.pointer(main, PointerKind::Up, px, py, none);
      },
      Step::TreeClick => {
        let r = rect(
          row(boxes, (Role::Tree, "Layers"), Role::TreeItem, 0),
          "Layer 1",
        )?;
        // On the name, not the arrow.
        cx.click(main, r.x + 80.0, r.y + r.height / 2.0);
      },
      Step::TreeOpen | Step::TreeInto => {
        cx.key(main, &key(NamedKey::ArrowRight));
      },
      Step::TreeOut | Step::TreeClose => {
        cx.key(main, &key(NamedKey::ArrowLeft));
      },
      Step::LogScroll => {
        let log = rect(find(boxes, Role::ListBox, "Log"), "the log")?;
        self.memory.log_top = first_index(boxes, (Role::ListBox, "Log"));
        let (x, y) = centre(log);
        cx.wheel(main, (x, y), (0.0, 26.0 * 500.0));
      },
      Step::Dark => {
        let (x, y) = centre(rect(find(boxes, Role::Button, "Dark"), "Dark")?);
        cx.click(main, x, y);
      },
    }
    Ok(())
  }

  /// Whether the step's result shows.
  fn check(
    &self,
    cx: &Context<'_>,
    main: WindowId,
    step: Step,
  ) -> Result<(), String> {
    let s = state();
    let boxes = cx.ui_boxes(main);
    let want = |ok: bool, what: String| if ok { Ok(()) } else { Err(what) };
    let current_part = || {
      s.parts
        .current()
        .and_then(|k| k.number())
        .map(|n| n as u32)
    };
    let order = s.order.get();
    match step {
      Step::Light => {
        let table = rows_made(boxes, (Role::Table, "Parts"), Role::Row);
        let log = rows_made(boxes, (Role::ListBox, "Log"), Role::ListBoxOption);
        let tree = rows_made(boxes, (Role::Tree, "Layers"), Role::TreeItem);
        let style = boxes
          .iter()
          .find_map(|b| element(b).and_then(|e| e.window_style));
        want(
          (10..200).contains(&table)
            && (5..200).contains(&log)
            && (10..=100).contains(&tree)
            && style == Some(window_style(&ThemeData::light())),
          format!("rows made: table {table}, log {log}, tree {tree}"),
        )
      },
      Step::TableClick => {
        let name = find(boxes, Role::TextInput, "Name")
          .map(|i| text_of(boxes, i))
          .unwrap_or_default();
        want(
          current_part() == Some(order[2])
            && s.parts.len() == 1
            && name == "U3",
          format!("current {:?}, inspector shows {name:?}", current_part()),
        )
      },
      Step::TableShift => want(
        s.parts.len() == 3 && current_part() == Some(order[4]),
        format!("{} selected", s.parts.len()),
      ),
      Step::TableAll => want(
        s.parts.len() == PARTS,
        format!("{} selected", s.parts.len()),
      ),
      Step::TableEnd => {
        let last = part_row(boxes, PARTS - 1).map(|i| boxes[i].rect);
        let body = find(boxes, Role::Table, "Parts").map(|i| boxes[i].rect);
        let shown = matches!((last, body), (Some(l), Some(b))
          if l.y >= b.y && l.y + l.height <= b.y + b.height + 0.5);
        want(
          current_part() == Some(order[PARTS - 1])
            && s.parts.len() == 1
            && shown,
          format!("last row shown: {shown}"),
        )
      },
      Step::TableHome => want(
        current_part() == Some(order[0]) && part_row(boxes, 0).is_some(),
        format!("{:?}", current_part()),
      ),
      Step::SortValue | Step::SortValueDown => {
        let down = step == Step::SortValueDown;
        let sorted = s.sort.get().is_some_and(|x| {
          x.column == 1 && (x.order == dovetail::SortOrder::Descending) == down
        });
        let (a, b) = (s.part(order[0]).value, s.part(order[1]).value);
        let header = find(boxes, Role::ColumnHeader, "Value")
          .and_then(|i| element(&boxes[i]))
          .and_then(|e| e.sorted);
        want(
          sorted && header.is_some() && if down { a >= b } else { a <= b },
          format!("sort {:?}, first values {a} {b}", s.sort.get()),
        )
      },
      Step::Resize => {
        let w = s.widths.get()[0];
        let cell =
          find(boxes, Role::ColumnHeader, "Name").map(|i| boxes[i].rect.width);
        want(
          (w - 150.0).abs() < 1.0 && cell == Some(w),
          format!("width {w}, header {cell:?}"),
        )
      },
      Step::InspectorCheck => {
        let part = self.memory.part.ok_or("no part")?;
        want(
          s.part(part).locked != self.memory.locked
            && s.history.undo_name().as_deref() == Some("Change Locked"),
          format!("locked {}", s.part(part).locked),
        )
      },
      Step::InspectorUndo => {
        let part = self.memory.part.ok_or("no part")?;
        want(
          s.part(part).locked == self.memory.locked,
          format!("locked {}", s.part(part).locked),
        )
      },
      Step::NumberUp | Step::NumberUpAgain => {
        let part = self.memory.part.ok_or("no part")?;
        let v = s.part(part).value;
        let steps = if step == Step::NumberUp { 0.5 } else { 1.0 };
        want(
          (v - (self.memory.value + steps)).abs() < 0.01,
          format!("value {v}"),
        )
      },
      Step::NumberCommit => want(
        s.history.undo_name().as_deref() == Some("Change Value"),
        format!("undo {:?}", s.history.undo_name()),
      ),
      Step::NumberUndo => {
        let part = self.memory.part.ok_or("no part")?;
        let v = s.part(part).value;
        want((v - self.memory.value).abs() < 0.01, format!("value {v}"))
      },
      Step::DragToPicked => {
        let picked = s.picked.get();
        let rows =
          rows_made(boxes, (Role::ListBox, "Picked"), Role::ListBoxOption);
        want(
          picked == vec![order[4], order[5]] && rows == 2,
          format!("picked {picked:?}, rows {rows}"),
        )
      },
      Step::TreeClick => want(
        s.layers.current() == Some(Key::from(0u64)),
        format!("{:?}", s.layers.current()),
      ),
      Step::TreeOpen => {
        let shapes = find(boxes, Role::TreeItem, "Shape 1.1").is_some();
        want(
          s.open.is_open(&Key::from(0u64)) && shapes,
          format!("open {}, shapes {shapes}", s.open.is_open(&Key::from(0u64))),
        )
      },
      Step::TreeInto => want(
        s.layers.current() == Some(crate::shape_key(0, 0)),
        format!("{:?}", s.layers.current()),
      ),
      Step::TreeOut => want(
        s.layers.current() == Some(Key::from(0u64)),
        format!("{:?}", s.layers.current()),
      ),
      Step::TreeClose => want(
        !s.open.is_open(&Key::from(0u64))
          && find(boxes, Role::TreeItem, "Shape 1.1").is_none(),
        "still open".into(),
      ),
      Step::LogScroll => {
        let top = first_index(boxes, (Role::ListBox, "Log"));
        let heights = log_heights(boxes);
        want(
          top > self.memory.log_top + 100 && heights.0 > heights.1 + 5.0,
          format!("first row {top}, tall and short rows {heights:?}"),
        )
      },
      Step::Dark => {
        let t = ThemeData::dark();
        let style = boxes
          .iter()
          .find_map(|b| element(b).and_then(|e| e.window_style));
        let look = t.row_look();
        let header = boxes
          .iter()
          .filter_map(element)
          .any(|e| e.style.background == look.header);
        want(
          style == Some(window_style(&t)) && header,
          format!(
            "dark style: {}, header: {header}",
            style == Some(window_style(&t))
          ),
        )
      },
    }
  }
}

/// The first log row made.
fn first_index(
  boxes: &[LayoutBox],
  view: (Role, &str),
) -> usize {
  find(boxes, view.0, view.1).map_or(0, |v| {
    inside(boxes, v)
      .into_iter()
      .filter_map(|i| element(&boxes[i]))
      .filter(|e| e.role == Role::ListBoxOption)
      .filter_map(|e| e.item.map(|p| p.index))
      .min()
      .unwrap_or(0)
  })
}

/// The heights of a two-line and a one-line row in the log.
fn log_heights(boxes: &[LayoutBox]) -> (f32, f32) {
  let Some(v) = find(boxes, Role::ListBox, "Log") else {
    return (0.0, 0.0);
  };
  let mut tall = 0.0f32;
  let mut short = f32::MAX;
  for i in inside(boxes, v) {
    if let Some(e) = element(&boxes[i])
      && e.role == Role::ListBoxOption
      && let Some(p) = e.item
    {
      let h = boxes[i].rect.height;
      if p.index.is_multiple_of(5) {
        tall = tall.max(h);
      } else {
        short = short.min(h);
      }
    }
  }
  (tall, short)
}

impl Application for Test {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let options = WindowOptions::new("Data views self-test")
      .with_size(LogicalSize::new(1180.0, 760.0));
    match cx.open_ui_window(options, ui! { crate::App {} }) {
      Ok(id) => self.main = Some(id),
      Err(e) => {
        println!("FAIL  could not open the window: {e}");
        cx.exit();
      },
    }
    cx.wake_after(Duration::from_secs(90));
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
    if STATE.get().is_none() {
      return;
    }
    let Some(&step) = STEPS.get(self.at) else {
      return;
    };
    if !self.acted {
      if let Err(e) = self.act(cx, main, step) {
        println!("FAIL  {}: {e}", step.says());
        cx.exit();
        return;
      }
      self.acted = true;
      cx.request_redraw(main);
      return;
    }
    match self.check(cx, main, step) {
      Ok(()) => {
        println!("PASS  {}", step.says());
        self.at += 1;
        self.acted = false;
        if self.at == STEPS.len() {
          PASSED.store(true, Ordering::Relaxed);
          println!("PASS  each view works with the pointer and the keyboard");
          cx.close_window(main);
          return;
        }
      },
      Err(e) => self.last = e,
    }
    cx.request_redraw(main);
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let waiting = STEPS.get(self.at).map_or("", |s| s.says());
    println!("FAIL  timed out waiting for: {waiting}");
    println!("      last: {}", self.last);
    cx.exit();
  }
}

pub fn run() -> ExitCode {
  let test = Test {
    main: None,
    at: 0,
    acted: false,
    memory: Memory::default(),
    last: String::new(),
  };
  let started = Instant::now();
  let result = dovetail::desktop::run(test);
  let _ = started;
  match result {
    Ok(()) if PASSED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("data: {e}");
      ExitCode::FAILURE
    },
  }
}
