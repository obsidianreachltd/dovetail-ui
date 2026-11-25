//! M11 example: a small board of parts, kept as a document.
//!
//! - `cargo run -p example-documents` opens it. Add, move, rename and
//!   delete parts; every edit is a command, so it can be undone. Save,
//!   Save As and Open use the system's file dialogs. The title shows `*`
//!   while there are unsaved changes, and closing asks first. Unsaved work
//!   is kept in the user's settings folder (or the folder named by
//!   `DOVETAIL_RECOVERY_DIR`), and offered back if the program stops
//!   without saving. "Check board" is a long task with progress and
//!   Cancel.
//! - `cargo run -p example-documents -- --self-test` does all of that by
//!   itself, and then checks recovery by stopping a copy of itself in the
//!   middle of editing.

mod self_test;

use std::cell::Cell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use dovetail::commands::{CommandButton, CommandScope, Commands, Toolbar};
use dovetail::documents::{
  Document, DocumentFrame, DocumentKind, Recovery, TaskProgress,
};
use dovetail::prelude::*;
use dovetail::{
  Align, Border, Edges, Element, ElementKind, Progress, Role, Size, Style,
  Task, TaskEnd, spawn_task,
};

/// A part on the board.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Part {
  pub(crate) name: String,
  pub(crate) x: i32,
  pub(crate) y: i32,
}

/// The document: the parts, in the order they were added.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Board {
  pub(crate) parts: Vec<Part>,
}

const HEADER: &str = "dovetail-board 1";

impl Board {
  /// The board's file: a header line, then one line per part, as
  /// `part X Y NAME`.
  pub(crate) fn to_text(&self) -> String {
    let mut text = format!("{HEADER}\n");
    for p in &self.parts {
      text.push_str(&format!("part {} {} {}\n", p.x, p.y, p.name));
    }
    text
  }

  pub(crate) fn from_text(text: &str) -> Result<Board, String> {
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
      return Err("this is not a board file".into());
    }
    let mut parts = Vec::new();
    for (n, line) in lines.enumerate() {
      if line.trim().is_empty() {
        continue;
      }
      let mut words = line.splitn(4, ' ');
      let part = match (words.next(), words.next(), words.next(), words.next())
      {
        (Some("part"), Some(x), Some(y), Some(name)) => {
          match (x.parse(), y.parse()) {
            (Ok(x), Ok(y)) => Part { name: name.to_owned(), x, y },
            _ => {
              return Err(format!(
                "line {}: the position is not a number",
                n + 2
              ));
            },
          }
        },
        _ => return Err(format!("line {}: not a part", n + 2)),
      };
      parts.push(part);
    }
    Ok(Board { parts })
  }
}

fn kind() -> DocumentKind<Board> {
  DocumentKind::new(
    "Board",
    &["board"],
    Board::default,
    |b: &Board| b.to_text().into_bytes(),
    |bytes| {
      let text = std::str::from_utf8(bytes)
        .map_err(|_| "the file is not text".to_owned())?;
      Board::from_text(text)
    },
  )
}

/// What the window keeps outside the document: which part is chosen, the
/// name being typed, and the check in progress. The self-test reads it.
#[derive(Clone, Copy)]
pub(crate) struct Parts {
  pub(crate) doc: Document<Board>,
  pub(crate) commands: Commands,
  pub(crate) selected: Signal<Option<usize>>,
  pub(crate) name: Signal<String>,
  pub(crate) status: Signal<String>,
  pub(crate) check: Signal<Option<(Task, Option<Progress>)>>,
}

thread_local! {
  /// The window's parts, for the self-test.
  pub(crate) static PARTS: Cell<Option<Parts>> = const { Cell::new(None) };
}

/// How long each step of "Check board" takes.
pub(crate) const CHECK_STEP: Duration = Duration::from_millis(40);
/// How many steps "Check board" has.
pub(crate) const CHECK_STEPS: u32 = 40;

fn define_commands(p: Parts) -> Commands {
  let doc = p.doc;
  let next_name = move || {
    let n = doc.read(|b| {
      b.parts
        .iter()
        .filter_map(|p| p.name.strip_prefix('R')?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
    });
    format!("R{}", n + 1)
  };
  let chosen = move || {
    let count = doc.read(|b| b.parts.len());
    p.selected.get().filter(|i| *i < count)
  };
  let commands = doc.file().add_commands(p.commands);
  commands
    .define("board.add", "Add part", Some("Mod+I"), move || {
      let name = next_name();
      let count = doc.read(|b| b.parts.len()) as i32;
      doc.edit(format!("Add {name}"), |b| {
        b.parts.push(Part {
          name,
          x: 10 * (count + 1),
          y: 10 * (count + 1),
        });
      });
      p.selected.set(Some(count as usize));
    })
    .define("board.delete", "Delete part", Some("Delete"), move || {
      let Some(i) = chosen() else {
        return;
      };
      let name = doc.read(|b| b.parts[i].name.clone());
      doc.edit(format!("Delete {name}"), move |b| {
        b.parts.remove(i);
      });
      p.selected.set(None);
    })
    .enabled_when("board.delete", move || chosen().is_some())
    .define("board.move", "Move right", Some("Mod+M"), move || {
      let Some(i) = chosen() else {
        return;
      };
      let name = doc.read(|b| b.parts[i].name.clone());
      doc.edit(format!("Move {name}"), move |b| b.parts[i].x += 5);
    })
    .enabled_when("board.move", move || chosen().is_some())
    .define("board.rename", "Rename", None, move || {
      let Some(i) = chosen() else {
        return;
      };
      let new = p.name.get().trim().to_owned();
      if new.is_empty() || new.contains('\n') {
        return;
      }
      doc.edit(format!("Rename to {new}"), move |b| b.parts[i].name = new);
    })
    .enabled_when("board.rename", move || chosen().is_some())
    .define("board.check", "Check board", None, move || check(p))
    .enabled_when("board.check", move || p.check.map(Option::is_none))
}

/// The long task: looks at every part in turn, slowly, on another thread.
fn check(p: Parts) {
  let board = p.doc.get();
  let task = spawn_task(
    move |cx| {
      for step in 0..CHECK_STEPS {
        let part = &board
          .parts
          .get(step as usize % board.parts.len().max(1));
        let doing = part.map_or_else(
          || "Looking at the board".to_owned(),
          |p| format!("Looking at {}", p.name),
        );
        cx.progress(step as f32 / CHECK_STEPS as f32, doing);
        if !cx.sleep(CHECK_STEP) {
          return None;
        }
      }
      Some(board.parts.len())
    },
    Some(Callback::new(move |progress| {
      p.check.update(|c| {
        if let Some(c) = c {
          c.1 = Some(progress);
        }
      })
    })),
    Callback::new(move |end| {
      p.check.set(None);
      p.status.set(match end {
        TaskEnd::Done(n) => {
          format!("Board checked: {n} parts, no problems found.")
        },
        TaskEnd::Cancelled => "The check was cancelled.".to_owned(),
        TaskEnd::Failed(e) => format!("The check failed: {e}"),
      });
    }),
  );
  p.check.set(Some((task, None)));
  p.status.set("Checking the board...".into());
}

fn part_row(
  p: Parts,
  i: usize,
  part: &Part,
) -> View {
  let chosen = p.selected.get() == Some(i);
  let name = part.name.clone();
  Element::new(ElementKind::Row)
    .style(Style {
      padding: Edges::xy(8.0, 4.0),
      gap: 12.0,
      radius: 4.0,
      background: if chosen { Color::hex(0xdce6fb) } else { Color::WHITE },
      ..Style::default()
    })
    .role(Role::ListBoxOption)
    .toggled(chosen)
    .focusable(true)
    .on_press(Callback::new(move |()| {
      p.selected.set(Some(i));
      p.name.set(name.clone());
    }))
    .children([
      View::text(part.name.clone()),
      ui! { Text { value: format!("at {}, {}", part.x, part.y), color: Some(Color::hex(0x6c6f85)) } },
    ])
    .into()
}

#[component]
fn App(#[prop(default)] recovery: Option<Recovery>) -> View {
  let doc = use_state(|| Document::new(kind())).get();
  let selected = use_state(|| None::<usize>);
  let name = use_state(String::new);
  let status = use_state(|| "Ready.".to_string());
  let check = use_state(|| None::<(Task, Option<Progress>)>);
  let commands = use_state(Commands::new).get();
  let parts = Parts {
    doc,
    commands,
    selected,
    name,
    status,
    check,
  };
  use_state(move || {
    define_commands(parts);
    if let Some(r) = recovery {
      doc.file().keep_recovery(r);
    }
  });
  PARTS.set(Some(parts));

  let toolbar = ui! {
    Toolbar {
      CommandButton { commands: commands, id: "file.new" }
      CommandButton { commands: commands, id: "file.open" }
      CommandButton { commands: commands, id: "file.save" }
      CommandButton { commands: commands, id: "file.save_as" }
      CommandButton { commands: commands, id: "edit.undo" }
      CommandButton { commands: commands, id: "edit.redo" }
    }
  };
  let board_tools = ui! {
    Toolbar {
      CommandButton { commands: commands, id: "board.add" }
      CommandButton { commands: commands, id: "board.delete" }
      CommandButton { commands: commands, id: "board.move" }
      CommandButton { commands: commands, id: "board.check" }
    }
  };
  let board = doc.get();
  let rows: Vec<View> = board
    .parts
    .iter()
    .enumerate()
    .map(|(i, part)| part_row(parts, i, part))
    .collect();
  let list: View = Element::new(ElementKind::Column)
    .style(Style {
      gap: 2.0,
      padding: Edges::all(6.0),
      width: Size::Fixed(260.0),
      height: Size::Fill,
      align: Align::Stretch,
      border: Border { width: 1.0, color: Color::hex(0xdce0e8) },
      radius: 6.0,
      scroll: true,
      ..Style::default()
    })
    .role(Role::ListBox)
    .label("Parts")
    .children(if rows.is_empty() {
      vec![View::text("No parts yet. Add one.")]
    } else {
      rows
    })
    .into();
  let inspector = ui! {
    Column {
      Text { value: "Name of the chosen part", size: Some(13.0) }
      TextField {
        value: name.get(),
        label: "Part name",
        width: 200.0,
        on_change: move |v| name.set(v),
      }
      CommandButton { commands: commands, id: "board.rename" }
    }
  };
  let main: View = Element::new(ElementKind::Row)
    .style(Style {
      gap: 16.0,
      padding: Edges::all(10.0),
      grow: 1.0,
      width: Size::Fill,
      ..Style::default()
    })
    .children([list, inspector])
    .into();
  let status_line = ui! { Text { value: status.get(), size: Some(13.0) } };
  let footer: View = Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::xy(10.0, 6.0),
      ..Style::default()
    })
    .children([status_line])
    .into();
  let mut content = vec![toolbar, board_tools, main, footer];
  if let Some((task, progress)) = check.get() {
    content.push(ui! {
      TaskProgress {
        title: "Checking the board",
        progress: progress,
        on_cancel: move |()| task.cancel(),
      }
    });
  }
  let body: View = Element::new(ElementKind::Column)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      align: Align::Stretch,
      ..Style::default()
    })
    .children(content)
    .into();
  ui! {
    DocumentFrame {
      file: doc.file(),
      app: "Board",
      CommandScope { commands: commands, { body } }
    }
  }
}

/// Where unsaved work is kept.
fn recovery_dir() -> Option<PathBuf> {
  if let Some(dir) = std::env::var_os("DOVETAIL_RECOVERY_DIR") {
    return Some(PathBuf::from(dir));
  }
  dovetail::desktop::settings_dir("dovetail-example-documents")
    .map(|d| d.join("recovery"))
}

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().collect();
  if let Some(code) = self_test::run(&args) {
    return code;
  }
  let recovery = recovery_dir().map(Recovery::new);
  if let Some(r) = &recovery {
    println!("documents: unsaved work is kept in {}", r.dir().display());
  }
  match dovetail::run("Board", ui! { App { recovery: recovery } }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("documents: {e}");
      ExitCode::FAILURE
    },
  }
}
