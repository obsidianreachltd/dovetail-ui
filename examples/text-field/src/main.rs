//! M06 example: a text field on one line and one on several lines.
//!
//! - `cargo run -p example-text-field` opens it. Click a field and type.
//! - `cargo run -p example-text-field -- --self-test` types, selects and
//!   undoes, copies, pastes and composes with an input method by itself,
//!   then moves the caret and selects in text that mixes English and
//!   Arabic, and checks that Arabic lines up on the right (M13). It checks
//!   what the window shows, and exits with a failure code if something is
//!   wrong (M06 "Done when" item 2, M13 item 3).

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dovetail::desktop::{
  Application, Context, Frame, KeyEvent, Modifiers, NamedKey, WindowId,
  WindowOptions,
};
use dovetail::layout::{Content, LayoutBox, cursor_geometry, first_text};
use dovetail::prelude::*;
use dovetail::render::font_measure;
use dovetail::{Edges, Element, ElementKind, ImeEvent, PointerKind, Style};

#[component]
fn Form() -> View {
  let name = use_state(String::new);
  let notes = use_state(String::new);
  let greeting = name.map(|n| {
    if n.is_empty() {
      "Type your name.".to_owned()
    } else {
      format!("Hello, {n}!")
    }
  });
  ui! {
    Column {
      gap: 12.0,
      Text { value: greeting }
      TextField { value: name.get(), on_change: move |v| name.set(v), label: "Name" }
      Text { value: "Notes" }
      TextField { value: notes.get(), on_change: move |v| notes.set(v), multiline: true, width: 320.0, label: "Notes" }
      Text { value: notes.map(|n| match n.split('\n').count() {
        1 => "1 line".to_owned(),
        lines => format!("{lines} lines"),
      }) }
    }
  }
}

#[component]
fn App() -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::all(24.0),
      ..Style::default()
    })
    .children([ui! { Form {} }])
    .into()
}

fn main() -> ExitCode {
  if std::env::args().any(|a| a == "--self-test") {
    return self_test();
  }
  match dovetail::run("Text fields", ui! { App {} }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("text-field: {e}");
      ExitCode::FAILURE
    },
  }
}

// --- Self-test ---------------------------------------------------------------

/// What each step waits to see before doing the next thing.
const STEPS: &[&str] = &[
  "Type your name.",
  "Hello, Ada!",
  "Hello, Grace!",
  "Hello, Ada!",
  "2 lines",
  "Hello, one two!",
  "one twoにほん",
  "Hello, one two日本!",
  MIXED_GREETING,
  ARABIC_GREETING,
];

/// English and Arabic on one line: "ab", "salam", "cd".
const MIXED: &str = "ab \u{0633}\u{0644}\u{0627}\u{0645} cd";
const MIXED_GREETING: &str = "Hello, ab \u{0633}\u{0644}\u{0627}\u{0645} cd!";
/// Arabic only: "salam alaykum".
const ARABIC: &str =
  "\u{0633}\u{0644}\u{0627}\u{0645} \u{0639}\u{0644}\u{064A}\u{0643}\u{0645}";
const ARABIC_GREETING: &str = "Hello, \u{0633}\u{0644}\u{0627}\u{0645} \u{0639}\u{0644}\u{064A}\u{0643}\u{0645}!";

struct SelfTest {
  window: Option<WindowId>,
  step: usize,
  /// The caret's halves seen while waiting for it to blink.
  blinks: Vec<bool>,
  /// What was on the clipboard before, put back at the end.
  saved: Option<String>,
  /// Walking Right through the mixed text: each caret place and its x.
  walk: Option<Vec<(usize, f32)>>,
}

/// The name field's caret, its x on the screen, and how many pieces its
/// selection has.
fn name_caret(boxes: &[LayoutBox]) -> Option<(usize, f32, usize)> {
  let index = boxes.iter().position(
    |b| matches!(&b.content, Content::Element(e) if e.text_cursor.is_some()),
  )?;
  let Content::Element(e) = &boxes[index].content else {
    return None;
  };
  let mut cursor = e.text_cursor?;
  cursor.show_caret = true;
  let text = first_text(boxes, index)?;
  let geometry = cursor_geometry(&boxes[text], &cursor, &font_measure());
  Some((cursor.caret, geometry.caret?.x, geometry.selection.len()))
}

fn find_text<'a>(
  boxes: &'a [LayoutBox],
  wanted: &str,
) -> Option<&'a LayoutBox> {
  boxes.iter().find(
    |b| matches!(&b.content, Content::Text { text, .. } if &**text == wanted),
  )
}

/// The fields, in order.
fn fields(boxes: &[LayoutBox]) -> Vec<&LayoutBox> {
  boxes
    .iter()
    .filter(
      |b| matches!(&b.content, Content::Element(e) if e.text_cursor.is_some()),
    )
    .collect()
}

fn type_text(
  cx: &mut Context<'_>,
  window: WindowId,
  text: &str,
) -> bool {
  text.chars().all(|c| {
    let event = if c == '\n' {
      KeyEvent::named(NamedKey::Enter)
    } else {
      KeyEvent::character(c)
    };
    cx.key(window, &event)
  })
}

impl Application for SelfTest {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let options = WindowOptions::new("Text field self-test");
    match cx.open_ui_window(options, ui! { App {} }) {
      Ok(id) => self.window = Some(id),
      Err(e) => {
        println!("FAIL  could not open the window: {e}");
        cx.exit();
      },
    }
    cx.wake_after(Duration::from_secs(15));
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
    window: WindowId,
  ) {
    // After the steps: the caret blinks, off and on again, by itself.
    if self.step == STEPS.len() {
      match cx.caret_blink(window) {
        None => {
          println!(
            "SKIP  the caret does not blink: the window is not in front"
          );
        },
        Some(on) => {
          if self.blinks.last() != Some(&on) {
            self.blinks.push(on);
          }
          if !self.blinks.windows(2).any(|w| w == [false, true]) {
            return;
          }
          println!("PASS  the caret blinks by itself");
        },
      }
      self.step += 1;
      if let Some(saved) = self.saved.take() {
        dovetail::set_clipboard_text(&saved);
      }
      PASSED.store(true, Ordering::Relaxed);
      println!(
        "PASS  typing, selecting, undo, Enter, copy, paste, the input method and the caret work"
      );
      cx.close_window(window);
      return;
    }
    // Walking Right through the mixed text, one place a frame.
    if let Some(walk) = &mut self.walk {
      let Some((caret, x, _)) = name_caret(cx.ui_boxes(window)) else {
        return;
      };
      if walk.last().map(|w| w.0) != Some(caret) {
        walk.push((caret, x));
        cx.key(window, &KeyEvent::named(NamedKey::ArrowRight));
        cx.request_redraw(window);
        return;
      }
      // Right did nothing: the end of the line.
      let walk = self.walk.take().unwrap();
      let stops = dovetail::text::grapheme_boundaries(MIXED).len();
      let mut places: Vec<usize> = walk.iter().map(|w| w.0).collect();
      places.sort_unstable();
      places.dedup();
      let rightward = walk.windows(2).all(|w| w[1].1 >= w[0].1 - 0.01);
      if places.len() != stops || places.len() != walk.len() || !rightward {
        println!(
          "FAIL  Right did not visit every place left to right: {walk:?}"
        );
        cx.exit();
        return;
      }
      println!(
        "PASS  Right visits all {stops} caret places in mixed text once each, moving right on the screen"
      );
      // Select from the start with Shift+Right until the selection holds
      // "ab " and the first Arabic letter, which is shown at the Arabic's
      // right end: Right goes through the Arabic from its left end, so
      // that takes seven presses.
      let shift = Modifiers { shift: true, ..Modifiers::default() };
      cx.key(window, &KeyEvent::named(NamedKey::Home));
      for _ in 0..7 {
        cx.key(window, &KeyEvent::named(NamedKey::ArrowRight).with(shift));
      }
      self.step = 100;
      cx.request_redraw(window);
      return;
    }
    if self.step == 100 {
      let Some((_, _, pieces)) = name_caret(cx.ui_boxes(window)) else {
        return;
      };
      if pieces == 0 {
        return;
      }
      if pieces < 2 {
        println!(
          "FAIL  the selection across the change of direction has {pieces} piece"
        );
        cx.exit();
        return;
      }
      println!(
        "PASS  selecting across the change of direction shows {pieces} pieces"
      );
      // Now Arabic only, which lines up on the right of the field.
      let all = Modifiers {
        control: !cfg!(target_os = "macos"),
        meta: cfg!(target_os = "macos"),
        ..Modifiers::default()
      };
      let mut select_all = KeyEvent::character('a').with(all);
      select_all.text = None;
      cx.key(window, &select_all);
      type_text(cx, window, ARABIC);
      self.step = STEPS.len() - 1;
      return;
    }
    let Some(expected) = STEPS.get(self.step) else {
      return;
    };
    let boxes = cx.ui_boxes(window);
    if find_text(boxes, expected).is_none() {
      // An earlier frame; a later one shows the change.
      return;
    }
    println!("PASS  the window shows \"{expected}\"");
    if *expected == ARABIC_GREETING {
      // The Arabic name lines up on the right of the field.
      let field = fields(boxes)[0].rect;
      let Some(text) = find_text(boxes, ARABIC) else {
        println!("FAIL  the Arabic name is not shown");
        cx.exit();
        return;
      };
      let gap = field.x + field.width - (text.rect.x + text.rect.width);
      if !(0.0..=12.0).contains(&gap) {
        println!(
          "FAIL  the Arabic name is not on the right: {gap} pixels from it"
        );
        cx.exit();
        return;
      }
      println!(
        "PASS  Arabic lines up on the right of the field ({gap:.0} pixels in)"
      );
    }
    let name = fields(boxes)[0].rect;
    let (left, middle) = (name.x + 4.0, name.y + name.height / 2.0);
    let right = name.x + name.width - 4.0;
    let command = Modifiers {
      control: !cfg!(target_os = "macos"),
      meta: cfg!(target_os = "macos"),
      ..Modifiers::default()
    };
    self.step += 1;
    let ok = match self.step {
      1 => {
        // Click the name field to focus it, then type.
        cx.click(window, left, middle);
        type_text(cx, window, "Ada")
      },
      2 => {
        // Drag across the whole name to select it, then type over it.
        let up = Modifiers::default();
        cx.pointer(window, PointerKind::Down, left, middle, up);
        cx.pointer(window, PointerKind::Move, right, middle, up);
        cx.pointer(window, PointerKind::Up, right, middle, up);
        type_text(cx, window, "Grace")
      },
      3 => {
        // Undo the typing over the selection.
        let mut undo = KeyEvent::character('z').with(command);
        undo.text = None;
        cx.key(window, &undo)
      },
      4 => {
        // Tab to the notes and type two lines.
        cx.key(window, &KeyEvent::named(NamedKey::Tab))
          && type_text(cx, window, "one\ntwo")
      },
      5 => {
        // Copy the notes with the system clipboard, Tab round to the name,
        // and paste over it. The line break becomes a space.
        self.saved = dovetail::clipboard_text();
        let shortcut = |c| {
          let mut event = KeyEvent::character(c).with(command);
          event.text = None;
          event
        };
        cx.key(window, &shortcut('a'))
          && cx.key(window, &shortcut('c'))
          && cx.key(window, &KeyEvent::named(NamedKey::Tab))
          && cx.key(window, &shortcut('a'))
          && cx.key(window, &shortcut('v'))
      },
      6 => {
        // The focused field takes text, so the input method is on. Compose
        // as a Japanese input method would.
        let kana = "にほん";
        if !cx.ime_allowed(window) {
          println!("FAIL  the input method is off in a focused text field");
          false
        } else {
          cx.ime(
            window,
            ImeEvent::Preedit {
              text: kana.into(),
              cursor: Some((kana.len(), kana.len())),
            },
          )
        }
      },
      7 => {
        // Convert and commit.
        cx.ime(
          window,
          ImeEvent::Preedit { text: String::new(), cursor: None },
        ) && cx.ime(window, ImeEvent::Commit("日本".into()))
      },
      8 => {
        // English and Arabic on one line.
        let mut select_all = KeyEvent::character('a').with(command);
        select_all.text = None;
        cx.key(window, &select_all) && type_text(cx, window, MIXED)
      },
      9 => {
        // Walk Right from the start, one place a frame.
        self.walk = Some(Vec::new());
        cx.key(window, &KeyEvent::named(NamedKey::Home));
        cx.request_redraw(window);
        true
      },
      // The last step was shown; the blink check comes next, on the
      // next frame.
      _ => {
        cx.request_redraw(window);
        true
      },
    };
    if !ok {
      println!("FAIL  a key was not used at step {}", self.step);
      cx.exit();
    }
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let waiting = STEPS
      .get(self.step)
      .copied()
      .unwrap_or("the caret to blink");
    println!("FAIL  timed out waiting for \"{waiting}\"");
    cx.exit();
  }
}

/// Set when every check passed; the application is gone by the time
/// `run` returns.
static PASSED: AtomicBool = AtomicBool::new(false);

fn self_test() -> ExitCode {
  let result = dovetail::desktop::run(SelfTest {
    window: None,
    step: 0,
    saved: None,
    blinks: Vec::new(),
    walk: None,
  });
  match result {
    Ok(()) if PASSED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("text-field: {e}");
      ExitCode::FAILURE
    },
  }
}
