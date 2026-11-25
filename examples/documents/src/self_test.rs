//! The self-test. In a window of its own it:
//!
//! - edits the board through commands, and undoes and redoes;
//! - saves with Save As, and opens the file again after Don't save;
//! - runs the long check, cancels it, and runs it to the end;
//! - asks to close with unsaved changes, cancels with Escape, and then
//!   saves and closes.
//!
//! Then it checks recovery: a copy of the program edits a board until its
//! work is kept, and is killed; a second copy finds the work and recovers
//! it.
//!
//! File dialogs are answered by the test, and every pointer and key event
//! is sent to the window by the program, not through the system, so
//! nothing reaches other windows.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::rc::Rc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dovetail::desktop::{
  Application, Context, Frame, KeyEvent, Modifiers, NamedKey, WindowId,
  WindowOptions,
};
use dovetail::documents::Recovery;
use dovetail::layout::{Content, LayoutBox, Rect};
use dovetail::prelude::*;
use dovetail::{FileDialog, FileDialogs, PointerKind, Role};

use crate::{Board, PARTS, Part, Parts};

/// What a copy of the program is for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
  /// The test in a window.
  Window,
  /// Edits until its work is kept, and waits to be killed.
  Crash,
  /// Finds the work the killed copy left, and recovers it.
  Recover,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
  Start,
  Edit,
  UndoRedo,
  SaveAs,
  AskBeforeOpen,
  DontSave,
  CheckRuns,
  CheckCancelled,
  CheckDone,
  CloseAsks,
  EscapeKeepsOpen,
  SaveAndClose,
  // The crashing copy.
  EditForCrash,
  // The recovering copy.
  Offered,
  Recovered,
}

impl Step {
  fn says(self) -> &'static str {
    match self {
      Step::Start => "a new board is untitled, with no unsaved changes",
      Step::Edit => {
        "adding, renaming and moving parts are commands, and the title shows *"
      },
      Step::UndoRedo => "Undo takes back the last edit, and Redo does it again",
      Step::SaveAs => "Save As saves the board, and the * goes",
      Step::AskBeforeOpen => "Open with unsaved changes asks first",
      Step::DontSave => {
        "Don't save opens the saved file, which gives the same board"
      },
      Step::CheckRuns => "the long check shows its progress",
      Step::CheckCancelled => "Cancel stops the check",
      Step::CheckDone => "the check runs to the end",
      Step::CloseAsks => "closing with unsaved changes asks first",
      Step::EscapeKeepsOpen => "Escape cancels, and the window stays open",
      Step::SaveAndClose => "Save saves the board, and the window closes",
      Step::EditForCrash => "the edits are kept for recovery",
      Step::Offered => "the work of the killed copy is offered back",
      Step::Recovered => "Recover gives back the board as it was edited",
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

/// The open dialog with this label, as the index of its box.
fn dialog(
  boxes: &[LayoutBox],
  title: &str,
) -> Option<usize> {
  boxes.iter().position(|b| {
    element(b).is_some_and(|e| {
      e.role == Role::Dialog && e.label.as_deref() == Some(title)
    })
  })
}

fn any_dialog(boxes: &[LayoutBox]) -> bool {
  boxes
    .iter()
    .any(|b| element(b).is_some_and(|e| e.role == Role::Dialog))
}

/// The button saying `text` inside the box `within`.
fn button_in(
  boxes: &[LayoutBox],
  within: usize,
  text: &str,
) -> Option<Rect> {
  let mut stack = vec![within];
  while let Some(i) = stack.pop() {
    if element(&boxes[i]).is_some_and(|e| e.role == Role::Button)
      && text_of(boxes, i) == text
    {
      return Some(boxes[i].rect);
    }
    stack.extend(boxes[i].children.iter().copied());
  }
  None
}

/// The progress bar's value, while one is shown.
fn progress(boxes: &[LayoutBox]) -> Option<f32> {
  boxes.iter().find_map(|b| {
    element(b)
      .filter(|e| e.role == Role::ProgressBar)
      .and_then(|e| e.value)
  })
}

fn click(
  cx: &mut Context<'_>,
  window: WindowId,
  r: Rect,
) {
  let (x, y) = (r.x + r.width / 2.0, r.y + r.height / 2.0);
  let none = Modifiers::default();
  cx.pointer(window, PointerKind::Down, x, y, none);
  cx.pointer(window, PointerKind::Up, x, y, none);
}

/// File dialogs that always choose `0`.
struct Choose(PathBuf);

impl FileDialogs for Choose {
  fn show(
    &self,
    _: FileDialog,
    done: Callback<Option<PathBuf>>,
  ) -> bool {
    done.call(Some(self.0.clone()));
    true
  }
}

/// The board the window should have saved when it closed.
static EXPECTED: Mutex<String> = Mutex::new(String::new());
static PASSED: AtomicBool = AtomicBool::new(false);

struct Test {
  mode: Mode,
  file: PathBuf,
  recovery: Option<Recovery>,
  main: Option<WindowId>,
  steps: Vec<Step>,
  at: usize,
  acted: bool,
  saved: Option<Board>,
}

fn part(
  name: &str,
  x: i32,
  y: i32,
) -> Part {
  Part { name: name.into(), x, y }
}

impl Test {
  fn act(
    &mut self,
    cx: &mut Context<'_>,
    main: WindowId,
    step: Step,
    p: Parts,
  ) -> bool {
    let boxes = cx.ui_boxes(main).to_vec();
    let c = p.commands;
    match step {
      Step::Start | Step::Offered => true,
      Step::Edit | Step::EditForCrash => {
        c.run("board.add");
        c.run("board.add");
        if step == Step::Edit {
          c.run("board.add");
        }
        p.selected.set(Some(0));
        p.name.set("C7".into());
        c.run("board.rename");
        c.run("board.move");
        true
      },
      Step::UndoRedo => {
        let before = p.doc.get();
        c.run("edit.undo");
        let undone = p.doc.get();
        c.run("edit.redo");
        if undone.parts[0].x + 5 != before.parts[0].x {
          println!("FAIL  Undo did not take back the move: {undone:?}");
          return false;
        }
        true
      },
      Step::SaveAs => c.run("file.save_as"),
      Step::AskBeforeOpen => {
        self.saved = Some(p.doc.get());
        c.run("board.add");
        c.run("file.open")
      },
      Step::DontSave => match dialog(&boxes, "Unsaved changes")
        .and_then(|d| button_in(&boxes, d, "Don't save"))
      {
        Some(r) => {
          click(cx, main, r);
          true
        },
        None => false,
      },
      Step::CheckRuns => c.run("board.check"),
      Step::CheckCancelled => {
        // Wait for some progress first.
        if !progress(&boxes).is_some_and(|v| v > 0.0) {
          return false;
        }
        match dialog(&boxes, "Checking the board")
          .and_then(|d| button_in(&boxes, d, "Cancel"))
        {
          Some(r) => {
            click(cx, main, r);
            true
          },
          None => false,
        }
      },
      Step::CheckDone => c.run("board.check"),
      Step::CloseAsks => {
        c.run("board.add");
        cx.request_close(main);
        true
      },
      Step::EscapeKeepsOpen => {
        cx.key(main, &KeyEvent::named(NamedKey::Escape));
        true
      },
      Step::SaveAndClose => {
        let Some(d) = dialog(&boxes, "Unsaved changes") else {
          // Asked again, as Escape answered the last time.
          cx.request_close(main);
          return false;
        };
        match button_in(&boxes, d, "Save") {
          Some(r) => {
            *EXPECTED.lock().unwrap() = p.doc.read(Board::to_text);
            PASSED.store(true, Ordering::Relaxed);
            click(cx, main, r);
            true
          },
          None => false,
        }
      },
      Step::Recovered => match dialog(&boxes, "Recover unsaved work")
        .and_then(|d| button_in(&boxes, d, "Recover"))
      {
        Some(r) => {
          click(cx, main, r);
          true
        },
        None => false,
      },
    }
  }

  fn check(
    &mut self,
    cx: &mut Context<'_>,
    main: WindowId,
    step: Step,
    p: Parts,
  ) -> bool {
    let boxes = cx.ui_boxes(main);
    let title = cx.window_title(main).unwrap_or_default();
    let file = p.doc.file();
    let board = p.doc.get();
    match step {
      Step::Start => title == "Untitled - Board" && !file.is_dirty(),
      Step::Edit | Step::UndoRedo => {
        board.parts
          == [
            part("C7", 15, 10),
            part("R2", 20, 20),
            part("R3", 30, 30),
          ]
          && title == "Untitled* - Board"
      },
      Step::SaveAs => {
        let on_disk = std::fs::read_to_string(&self.file).unwrap_or_default();
        title == format!("{} - Board", file.name())
          && !file.is_dirty()
          && on_disk == board.to_text()
      },
      Step::AskBeforeOpen => {
        dialog(boxes, "Unsaved changes").is_some() && board.parts.len() == 4
      },
      Step::DontSave => {
        let on_disk = std::fs::read_to_string(&self.file).unwrap_or_default();
        Some(&board) == self.saved.as_ref()
          && Board::from_text(&on_disk).as_ref() == Ok(&board)
          && !file.is_dirty()
          && !any_dialog(boxes)
      },
      Step::CheckRuns => dialog(boxes, "Checking the board").is_some(),
      Step::CheckCancelled => {
        p.status.get() == "The check was cancelled." && !any_dialog(boxes)
      },
      Step::CheckDone => {
        p.status.get() == "Board checked: 3 parts, no problems found."
          && !any_dialog(boxes)
      },
      Step::CloseAsks => {
        dialog(boxes, "Unsaved changes").is_some() && cx.window_count() == 1
      },
      Step::EscapeKeepsOpen => !any_dialog(boxes) && cx.window_count() == 1,
      // The window closes, so this is checked after the event loop ends.
      Step::SaveAndClose => false,
      Step::EditForCrash => {
        if file.snapshot_file().is_none() {
          return false;
        }
        // The parent reads this, and kills this copy.
        println!("SNAPSHOT-WRITTEN");
        true
      },
      Step::Offered => dialog(boxes, "Recover unsaved work").is_some(),
      Step::Recovered => {
        board.parts == [part("C7", 15, 10), part("R2", 20, 20)]
          && file.is_dirty()
          && !any_dialog(boxes)
      },
    }
  }
}

impl Application for Test {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    // The test answers the file dialogs.
    dovetail::set_file_dialogs(Some(Rc::new(Choose(self.file.clone()))));
    let options = WindowOptions::new("Board")
      .with_size(dovetail::desktop::LogicalSize::new(760.0, 520.0));
    let recovery = self.recovery.clone();
    match cx.open_ui_window(options, ui! { crate::App { recovery: recovery } })
    {
      Ok(id) => self.main = Some(id),
      Err(e) => {
        println!("FAIL  could not open the window: {e}");
        cx.exit();
      },
    }
    cx.wake_after(Duration::from_secs(if self.mode == Mode::Crash {
      120
    } else {
      40
    }));
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
    let (Some(main), Some(p)) = (self.main, PARTS.get()) else {
      return;
    };
    let Some(&step) = self.steps.get(self.at) else {
      return;
    };
    if self.acted {
      if !self.check(cx, main, step, p) {
        // Tasks and snapshot timers finish on their own: look again soon.
        cx.request_redraw(main);
        return;
      }
      println!("PASS  {}", step.says());
      self.at += 1;
      self.acted = false;
      if self.at == self.steps.len() {
        match self.mode {
          // Waits to be killed.
          Mode::Crash => return,
          Mode::Recover => {
            PASSED.store(true, Ordering::Relaxed);
            cx.exit();
            return;
          },
          Mode::Window => {},
        }
      }
    }
    let Some(&step) = self.steps.get(self.at) else {
      return;
    };
    self.acted = self.act(cx, main, step, p);
    cx.request_redraw(main);
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let waiting = self.steps.get(self.at).map_or("", |s| s.says());
    println!("FAIL  timed out waiting for: {waiting}");
    if let Some(p) = PARTS.get() {
      println!("      the board is {:?}", p.doc.get());
    }
    PASSED.store(false, Ordering::Relaxed);
    cx.exit();
  }
}

fn run_window(
  mode: Mode,
  file: PathBuf,
  recovery: Option<Recovery>,
) -> bool {
  let steps = match mode {
    Mode::Window => vec![
      Step::Start,
      Step::Edit,
      Step::UndoRedo,
      Step::SaveAs,
      Step::AskBeforeOpen,
      Step::DontSave,
      Step::CheckRuns,
      Step::CheckCancelled,
      Step::CheckDone,
      Step::CloseAsks,
      Step::EscapeKeepsOpen,
      Step::SaveAndClose,
    ],
    Mode::Crash => vec![Step::Start, Step::EditForCrash],
    Mode::Recover => vec![Step::Offered, Step::Recovered],
  };
  let test = Test {
    mode,
    file,
    recovery,
    main: None,
    steps,
    at: 0,
    acted: false,
    saved: None,
  };
  match dovetail::desktop::run(test) {
    Ok(()) => PASSED.load(Ordering::Relaxed),
    Err(e) => {
      println!("FAIL  {e}");
      false
    },
  }
}

/// Recovery for the copies: snapshots written soon after edits stop.
fn quick(dir: &Path) -> Recovery {
  Recovery::new(dir)
    .with_delays(Duration::from_millis(150), Duration::from_secs(5))
}

/// Starts a copy that edits until its work is kept, kills it, and starts a
/// copy that recovers the work.
fn crash_check(dir: &Path) -> bool {
  let _ = std::fs::remove_dir_all(dir);
  let Ok(exe) = std::env::current_exe() else {
    println!("FAIL  the program's own path is not known");
    return false;
  };
  let mut child = match Command::new(&exe)
    .arg("--crash-child")
    .arg(dir)
    .stdout(Stdio::piped())
    .spawn()
  {
    Ok(c) => c,
    Err(e) => {
      println!("FAIL  the copy to kill did not start: {e}");
      return false;
    },
  };
  let stdout = child.stdout.take().expect("piped");
  let (tx, rx) = std::sync::mpsc::channel();
  std::thread::spawn(move || {
    for line in BufReader::new(stdout)
      .lines()
      .map_while(Result::ok)
    {
      if line.starts_with("PASS") || line.starts_with("FAIL") {
        println!("      copy: {line}");
      }
      if line == "SNAPSHOT-WRITTEN" {
        let _ = tx.send(());
      }
    }
  });
  let kept = rx.recv_timeout(Duration::from_secs(60)).is_ok();
  // Killed, as a crash would stop it: nothing is saved or cleaned up.
  let _ = child.kill();
  let _ = child.wait();
  if !kept {
    println!("FAIL  the copy did not keep its work in time");
    return false;
  }
  println!("PASS  the copy was killed in the middle of editing");
  let start = Instant::now();
  let status = Command::new(&exe)
    .arg("--recover-child")
    .arg(dir)
    .status();
  let recovered = status.is_ok_and(|s| s.success());
  if recovered {
    println!(
      "PASS  the next start recovered the work ({:.1} s)",
      start.elapsed().as_secs_f32()
    );
  } else {
    println!("FAIL  the next start did not recover the work");
  }
  let _ = std::fs::remove_dir_all(dir);
  recovered
}

/// Runs the self-test, or one of its copies, when the arguments ask for
/// it.
pub fn run(args: &[String]) -> Option<ExitCode> {
  let code = |ok: bool| {
    if ok {
      ExitCode::SUCCESS
    } else {
      ExitCode::FAILURE
    }
  };
  let temp = std::env::temp_dir();
  let id = std::process::id();
  match args.get(1).map(String::as_str) {
    Some("--self-test") => {
      let file = temp.join(format!("dovetail-documents-self-test-{id}.board"));
      let _ = std::fs::remove_file(&file);
      let mut ok = run_window(Mode::Window, file.clone(), None);
      if ok {
        let on_disk = std::fs::read_to_string(&file).unwrap_or_default();
        ok = on_disk == *EXPECTED.lock().unwrap();
        if ok {
          println!("PASS  {}", Step::SaveAndClose.says());
        } else {
          println!("FAIL  the closed window did not save the board");
        }
      }
      let _ = std::fs::remove_file(&file);
      let dir =
        temp.join(format!("dovetail-documents-self-test-{id}-recovery"));
      ok = crash_check(&dir) && ok;
      if ok {
        println!(
          "PASS  editing, saving, opening, closing, recovery and the long check work"
        );
      }
      Some(code(ok))
    },
    Some(flag @ ("--crash-child" | "--recover-child")) => {
      let dir = PathBuf::from(args.get(2)?);
      let mode = if flag == "--crash-child" {
        Mode::Crash
      } else {
        Mode::Recover
      };
      let file = dir.join("unused.board");
      Some(code(run_window(mode, file, Some(quick(&dir)))))
    },
    _ => None,
  }
}
