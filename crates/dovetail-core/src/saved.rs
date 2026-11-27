//! Windows' places and states, kept between runs as text (M19, D0029).
//!
//! The format follows the docking layout's (D0016). The first line names
//! it and its version; each window has one line:
//!
//! ```text
//! dovetail-windows 1
//! window "main" at 120 80 size 1024 700 scale 1.5 state maximised monitor "DISPLAY1" 0 0 3840 2160
//! window "settings" at - size 480 360 scale 1 state normal
//! ```
//!
//! `at -` is a window whose position the system does not give, as on
//! Wayland. The monitor is left out when it is not known.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use crate::screen::{SavedMonitor, SavedWindow, ScreenRect};
use crate::window::WindowState;

/// The first line of the text, with its version.
const HEADER: &str = "dovetail-windows 1";

/// Why saved text could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedReadError {
  /// The line it went wrong on, counting from 1.
  pub line: usize,
  /// What was wrong.
  pub message: String,
}

impl fmt::Display for SavedReadError {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    write!(f, "line {}: {}", self.line, self.message)
  }
}

impl std::error::Error for SavedReadError {}

#[derive(Default)]
struct Inner {
  windows: BTreeMap<String, SavedWindow>,
  /// The file kept up to date, and the text last written to it.
  file: Option<(PathBuf, String)>,
}

/// The saved places and states of an application's windows, each under a
/// name (M19). Make one with `use_state(SavedWindows::new).get()`, and give
/// it to each [`Window`](crate::Window) with a `name`: the window opens
/// where it was saved, and saves its place when it changes.
///
/// Copies share the same windows.
#[derive(Clone, Default)]
pub struct SavedWindows(Rc<RefCell<Inner>>);

impl PartialEq for SavedWindows {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    Rc::ptr_eq(&self.0, &other.0)
  }
}

impl fmt::Debug for SavedWindows {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.debug_map()
      .entries(self.0.borrow().windows.iter())
      .finish()
  }
}

impl SavedWindows {
  /// No saved windows.
  pub fn new() -> Self {
    Self::default()
  }

  /// Reads the windows kept in `path`, if the file is there and can be
  /// read, and from then on writes them there whenever one changes. A file
  /// that cannot be read gives no saved windows, is left as it is until a
  /// window changes, and the error is returned.
  ///
  /// # Errors
  ///
  /// When the file is there but its text cannot be read.
  pub fn keep_in_file(
    &self,
    path: impl Into<PathBuf>,
  ) -> Result<(), SavedReadError> {
    let path = path.into();
    let mut result = Ok(());
    let mut written = String::new();
    if let Ok(text) = std::fs::read_to_string(&path) {
      match Self::read(&text) {
        Ok(windows) => {
          written = text;
          self.0.borrow_mut().windows = windows;
        },
        Err(e) => result = Err(e),
      }
    }
    self.0.borrow_mut().file = Some((path, written));
    result
  }

  /// The window saved as `name`.
  pub fn get(
    &self,
    name: &str,
  ) -> Option<SavedWindow> {
    self.0.borrow().windows.get(name).cloned()
  }

  /// The names saved, in order.
  pub fn names(&self) -> Vec<String> {
    self.0.borrow().windows.keys().cloned().collect()
  }

  /// Saves `window` as `name`, and writes the file if one is kept. A
  /// minimised window keeps the state saved before.
  pub fn set(
    &self,
    name: &str,
    mut window: SavedWindow,
  ) {
    {
      let mut inner = self.0.borrow_mut();
      if window.state == WindowState::Minimised {
        window.state = inner
          .windows
          .get(name)
          .map_or(WindowState::Normal, |w| w.state);
      }
      if inner.windows.get(name) == Some(&window) {
        return;
      }
      inner.windows.insert(name.to_string(), window);
    }
    self.write();
  }

  /// Changes only the state saved as `name`, if a window is saved there.
  pub fn set_state(
    &self,
    name: &str,
    state: WindowState,
  ) {
    let Some(mut window) = self.get(name) else {
      return;
    };
    window.state = state;
    self.set(name, window);
  }

  /// Forgets the window saved as `name`.
  pub fn forget(
    &self,
    name: &str,
  ) {
    if self.0.borrow_mut().windows.remove(name).is_some() {
      self.write();
    }
  }

  fn write(&self) {
    let text = self.to_text();
    let mut inner = self.0.borrow_mut();
    if let Some((path, written)) = &mut inner.file
      && *written != text
    {
      if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
      }
      match std::fs::write(&*path, &text) {
        Ok(()) => *written = text,
        Err(e) => eprintln!(
          "dovetail: could not save the windows to {}: {e}",
          path.display()
        ),
      }
    }
  }

  /// The saved windows as text.
  pub fn to_text(&self) -> String {
    let mut out = format!("{HEADER}\n");
    for (name, w) in &self.0.borrow().windows {
      out.push_str("window ");
      out.push_str(&quote(name));
      match w.position {
        Some((x, y)) => out.push_str(&format!(" at {x} {y}")),
        None => out.push_str(" at -"),
      }
      out.push_str(&format!(
        " size {} {} scale {} state {}",
        w.size.0,
        w.size.1,
        w.scale,
        state_word(w.state)
      ));
      if let Some(m) = &w.monitor {
        let b = m.bounds;
        out.push_str(&format!(
          " monitor {} {} {} {} {}",
          quote(&m.name),
          b.x,
          b.y,
          b.width,
          b.height
        ));
      }
      out.push('\n');
    }
    out
  }

  /// Saved windows from text, as [`SavedWindows::to_text`] writes it.
  ///
  /// # Errors
  ///
  /// When the text is not in this format, or not in a version this
  /// release reads. The error names the line.
  pub fn from_text(text: &str) -> Result<SavedWindows, SavedReadError> {
    let windows = Self::read(text)?;
    let saved = SavedWindows::new();
    saved.0.borrow_mut().windows = windows;
    Ok(saved)
  }

  fn read(text: &str) -> Result<BTreeMap<String, SavedWindow>, SavedReadError> {
    let mut lines = text.lines().enumerate();
    let error = |line: usize, message: &str| SavedReadError {
      line: line + 1,
      message: message.to_string(),
    };
    match lines.next() {
      Some((_, first)) if first.trim() == HEADER => {},
      Some((_, first)) if first.trim().starts_with("dovetail-windows ") => {
        return Err(error(0, "a newer version of the format"));
      },
      _ => return Err(error(0, "not saved windows")),
    }
    let mut windows = BTreeMap::new();
    for (n, line) in lines {
      if line.trim().is_empty() {
        continue;
      }
      let words = tokens(line).map_err(|m| error(n, m))?;
      let (name, window) = read_window(&words).map_err(|m| error(n, &m))?;
      windows.insert(name, window);
    }
    Ok(windows)
  }
}

fn state_word(state: WindowState) -> &'static str {
  match state {
    WindowState::Normal | WindowState::Minimised => "normal",
    WindowState::Maximised => "maximised",
    WindowState::FullScreen => "full-screen",
  }
}

fn quote(text: &str) -> String {
  let mut out = String::from("\"");
  for c in text.chars() {
    if c == '"' || c == '\\' {
      out.push('\\');
    }
    out.push(c);
  }
  out.push('"');
  out
}

/// The words of a line: plain words, and quoted ones with `\"` and `\\`.
fn tokens(line: &str) -> Result<Vec<String>, &'static str> {
  let mut words = Vec::new();
  let mut chars = line.chars().peekable();
  while let Some(&c) = chars.peek() {
    if c.is_whitespace() {
      chars.next();
    } else if c == '"' {
      chars.next();
      let mut word = String::new();
      loop {
        match chars.next() {
          Some('\\') => {
            word.push(chars.next().ok_or("a line ends inside quotes")?)
          },
          Some('"') => break,
          Some(c) => word.push(c),
          None => return Err("a line ends inside quotes"),
        }
      }
      words.push(word);
    } else {
      let mut word = String::new();
      while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
          break;
        }
        word.push(c);
        chars.next();
      }
      words.push(word);
    }
  }
  Ok(words)
}

fn read_window(words: &[String]) -> Result<(String, SavedWindow), String> {
  let mut at = words.iter().map(String::as_str);
  let mut next = |what: &str| at.next().ok_or(format!("{what} is missing"));
  let number = |word: &str| -> Result<f32, String> {
    word
      .parse::<f32>()
      .ok()
      .filter(|n| n.is_finite())
      .ok_or(format!("{word:?} is not a number"))
  };
  if next("`window`")? != "window" {
    return Err("each line after the first starts with `window`".into());
  }
  let name = next("the name")?.to_string();
  let mut window = SavedWindow {
    position: None,
    size: (0.0, 0.0),
    scale: 1.0,
    state: WindowState::Normal,
    monitor: None,
  };
  let mut sized = false;
  while let Ok(key) = next("") {
    match key {
      "at" => {
        let x = next("the position")?;
        if x != "-" {
          let y = number(next("the position")?)?;
          window.position = Some((number(x)?, y));
        }
      },
      "size" => {
        let w = number(next("the width")?)?;
        let h = number(next("the height")?)?;
        if w <= 0.0 || h <= 0.0 {
          return Err("a size is more than nothing".into());
        }
        window.size = (w, h);
        sized = true;
      },
      "scale" => {
        let s = number(next("the scale")?)?;
        if s <= 0.0 {
          return Err("a scale is more than nothing".into());
        }
        window.scale = s;
      },
      "state" => {
        window.state = match next("the state")? {
          "normal" => WindowState::Normal,
          "maximised" => WindowState::Maximised,
          "full-screen" => WindowState::FullScreen,
          other => return Err(format!("{other:?} is not a state")),
        };
      },
      "monitor" => {
        let name = next("the monitor's name")?.to_string();
        let mut n = [0.0; 4];
        for v in &mut n {
          *v = number(next("the monitor's bounds")?)?;
        }
        window.monitor = Some(SavedMonitor {
          name,
          bounds: ScreenRect::new(n[0], n[1], n[2], n[3]),
        });
      },
      other => return Err(format!("{other:?} is not known here")),
    }
  }
  if !sized {
    return Err("the size is missing".into());
  }
  Ok((name, window))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn example() -> SavedWindows {
    let saved = SavedWindows::new();
    saved.set(
      "main",
      SavedWindow {
        position: Some((120.0, 80.5)),
        size: (1024.0, 700.0),
        scale: 1.5,
        state: WindowState::Maximised,
        monitor: Some(SavedMonitor {
          name: "\\\\.\\DISPLAY1 \"big\"".into(),
          bounds: ScreenRect::new(0.0, 0.0, 3840.0, 2160.0),
        }),
      },
    );
    saved.set(
      "settings",
      SavedWindow {
        position: None,
        size: (480.0, 360.0),
        scale: 1.0,
        state: WindowState::Normal,
        monitor: None,
      },
    );
    saved
  }

  #[test]
  fn saved_text_is_read_back_the_same() {
    let saved = example();
    let text = saved.to_text();
    assert!(text.starts_with("dovetail-windows 1\n"), "{text}");
    let back = SavedWindows::from_text(&text).expect("it reads");
    assert_eq!(back.names(), ["main", "settings"]);
    assert_eq!(back.get("main"), saved.get("main"));
    assert_eq!(back.get("settings"), saved.get("settings"));
    assert_eq!(back.to_text(), text);
  }

  #[test]
  fn a_minimised_window_keeps_the_state_it_had() {
    let saved = example();
    let mut main = saved.get("main").unwrap();
    main.state = WindowState::Minimised;
    saved.set("main", main);
    assert_eq!(saved.get("main").unwrap().state, WindowState::Maximised);
  }

  #[test]
  fn unreadable_text_says_where() {
    for (text, line) in [
      ("", 1),
      ("dovetail-dock 1\n", 1),
      ("dovetail-windows 2\n", 1),
      ("dovetail-windows 1\nwindow \"a\" size 10\n", 2),
      (
        "dovetail-windows 1\nwindow \"a\" size 10 10\nwindow \"b\" at 1 2\n",
        3,
      ),
      (
        "dovetail-windows 1\nwindow \"a\" size 10 10 state huge\n",
        2,
      ),
      ("dovetail-windows 1\nwindow \"a size 10 10\n", 2),
      ("dovetail-windows 1\nwindow \"a\" size 10 NaN\n", 2),
    ] {
      let e = SavedWindows::from_text(text).expect_err(text);
      assert_eq!(e.line, line, "{text:?}: {e}");
    }
  }

  #[test]
  fn a_file_is_kept_up_to_date_and_read_on_the_next_run() {
    let dir = std::env::temp_dir().join(format!(
      "dovetail-saved-{}-{:?}",
      std::process::id(),
      std::thread::current().id()
    ));
    let path = dir.join("windows.txt");
    let _ = std::fs::remove_file(&path);

    let first = SavedWindows::new();
    assert_eq!(first.keep_in_file(&path), Ok(()), "no file is no error");
    first.set("main", example().get("main").unwrap());
    let text = std::fs::read_to_string(&path).expect("written");
    assert_eq!(text, first.to_text());

    let next = SavedWindows::new();
    assert_eq!(next.keep_in_file(&path), Ok(()));
    assert_eq!(next.get("main"), first.get("main"));

    std::fs::write(&path, "rubbish").unwrap();
    let broken = SavedWindows::new();
    assert!(broken.keep_in_file(&path).is_err());
    assert_eq!(broken.names(), Vec::<String>::new());
    assert_eq!(
      std::fs::read_to_string(&path).unwrap(),
      "rubbish",
      "left as it was"
    );
    let _ = std::fs::remove_dir_all(&dir);
  }
}
