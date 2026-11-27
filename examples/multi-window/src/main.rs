//! M19 example: an application whose windows are components.
//!
//! - `cargo run -p example-multi-window` opens the main window. "New
//!   document" opens a window for each document, and "Settings" opens a
//!   modal settings window, which blocks the main window until it closes.
//!   F11 (Control+Command+F on macOS) or the button makes the main window
//!   full screen. The list shows the open windows; a click brings one to
//!   the front.
//! - Every window keeps its place, size and state between runs, in
//!   `windows.txt` in the application's settings folder. A window saved on
//!   a monitor that is gone opens on one that is there.
//! - Closing the main window ends the application on Windows and Linux. On
//!   macOS it keeps running, and a click on its Dock icon opens the main
//!   window again.
//! - `cargo run -p example-multi-window -- --self-test` checks all of this
//!   in real windows, with a file of its own.

mod self_test;

use std::path::PathBuf;
use std::process::ExitCode;

use dovetail::commands::{
  CommandButton, CommandScope, Commands, add_full_screen,
};
use dovetail::prelude::*;
use dovetail::{
  Edges, Element, ElementKind, HostWindow, SavedWindows, Style, WindowState,
  app_windows, bring_to_front,
};

/// The main window's title.
pub(crate) const MAIN: &str = "Multi-window";

/// The whole application: the main window, a window for each open
/// document, and the settings window while it is open. Their places are
/// kept in `saved_file`.
#[component]
pub(crate) fn App(saved_file: PathBuf) -> View {
  let saved = use_state(|| {
    let saved = SavedWindows::new();
    if let Err(e) = saved.keep_in_file(&saved_file) {
      eprintln!("multi-window: the saved windows could not be read: {e}");
    }
    saved
  })
  .get();
  let running = use_state(|| true);
  let state = use_state(|| WindowState::Normal);
  let documents = use_state(Vec::<u32>::new);
  let commands = use_state(|| add_full_screen(Commands::new(), state)).get();
  if !running.get() {
    // Every window goes with the main one.
    return View::Empty;
  }
  let windows = documents.get().into_iter().map(|n| {
    let window = ui! {
      Window {
        title: format!("Document {n}"),
        name: format!("document {n}"),
        saved: saved.clone(),
        size: (420.0, 300.0),
        min_size: (240.0, 160.0),
        on_close: move |()| documents.update(|d| d.retain(|x| *x != n)),
        Document { number: n }
      }
    };
    View::keyed(window, u64::from(n))
  });
  ui! {
    Window {
      title: MAIN,
      name: "main",
      saved: saved.clone(),
      size: (640.0, 460.0),
      min_size: (420.0, 320.0),
      state: state.get(),
      on_state: move |s| state.set(s),
      on_close: move |()| running.set(false),
      CommandScope {
        commands: commands,
        Main { commands: commands, documents: documents, state: state }
      }
    }
    { View::fragment(windows) }
  }
}

/// The main window's content.
#[component]
fn Main(
  commands: Commands,
  documents: Signal<Vec<u32>>,
  state: Signal<WindowState>,
) -> View {
  let next = use_state(|| 1u32);
  let clicks = use_state(|| 0u32);
  let settings = use_state(|| false);
  let open: Vec<View> = app_windows()
    .into_iter()
    .map(|w| {
      let id: HostWindow = w.id;
      let text = format!("Front: {}", w.title);
      ui! { Button { text: text, on_click: move |_| bring_to_front(id) } }
    })
    .collect();
  let new_document = move |_| {
    let n = next.get();
    next.set(n + 1);
    documents.update(|d| d.push(n));
  };
  padded(ui! {
    Column {
      gap: 10.0,
      Text { value: state.map(|s| format!("Main window: {s:?}")) }
      Row {
        gap: 8.0,
        Button { text: "New document", on_click: new_document }
        Button { text: "Settings", on_click: move |_| settings.set(true) }
        CommandButton { commands: commands, id: dovetail::commands::FULL_SCREEN }
      }
      Row {
        gap: 8.0,
        Button { text: "Click me", on_click: move |_| clicks.update(|n| *n += 1) }
        Text { value: clicks.map(|n| format!("Clicks: {n}")) }
      }
      Text { value: "Open windows:" }
      Column { gap: 4.0, { View::fragment(open) } }
      if settings.get() {
        Window {
          title: "Settings",
          size: (360.0, 220.0),
          resizable: false,
          modal: true,
          on_close: move |()| settings.set(false),
          { padded(ui! {
            Column {
              gap: 10.0,
              Text { value: "The main window waits until this one closes." }
              Button { text: "Done", on_click: move |_| settings.set(false) }
            }
          }) }
        }
      }
    }
  })
}

/// A document's window.
#[component]
fn Document(number: u32) -> View {
  let edits = use_state(|| 0u32);
  padded(ui! {
    Column {
      gap: 10.0,
      Text { value: format!("Document {number}") }
      Button { text: "Edit", on_click: move |_| edits.update(|n| *n += 1) }
      Text { value: edits.map(|n| format!("Edits: {n}")) }
    }
  })
}

fn padded(view: View) -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::all(16.0),
      ..Style::default()
    })
    .children([view])
    .into()
}

fn main() -> ExitCode {
  if std::env::args().any(|a| a == "--self-test") {
    return self_test::run();
  }
  let saved_file = dovetail::desktop::settings_dir("dovetail-multi-window")
    .unwrap_or_default()
    .join("windows.txt");
  match dovetail::run_windows(ui! { App { saved_file: saved_file } }) {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("multi-window: {e}");
      ExitCode::FAILURE
    },
  }
}
