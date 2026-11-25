//! File dialogs (M11).
//!
//! The desktop crate installs the system's own dialogs with
//! [`set_file_dialogs`] (D0017). Code asks for a file with
//! [`show_file_dialog`]; where no system dialog is available, it is told
//! so, and can show one of its own.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::view::Callback;

/// Whether a dialog chooses a file to open, or a name to save as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileDialogKind {
  /// Choose a file that exists.
  Open,
  /// Choose a name to save as. The dialog asks before choosing a file that
  /// exists.
  Save,
}

/// A kind of file the dialog offers, such as "Board files" with the
/// extension `board`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFilter {
  /// The name shown for it.
  pub name: String,
  /// Extensions without the dot, such as `"board"`. Empty means any file.
  pub extensions: Vec<String>,
}

impl FileFilter {
  /// A filter called `name` for files with these extensions.
  pub fn new(
    name: impl Into<String>,
    extensions: &[&str],
  ) -> FileFilter {
    FileFilter {
      name: name.into(),
      extensions: extensions
        .iter()
        .map(|e| e.trim_start_matches('.').to_owned())
        .collect(),
    }
  }

  /// True when `path` has one of the extensions, or the filter takes any
  /// file.
  pub fn accepts(
    &self,
    path: &std::path::Path,
  ) -> bool {
    self.extensions.is_empty()
      || path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| {
          self
            .extensions
            .iter()
            .any(|x| x.eq_ignore_ascii_case(e))
        })
  }
}

/// A file dialog to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDialog {
  /// Open or save.
  pub kind: FileDialogKind,
  /// The dialog's title.
  pub title: String,
  /// The folder to start in.
  pub folder: Option<PathBuf>,
  /// The name to start with, for saving.
  pub name: Option<String>,
  /// The kinds of file offered, the first one chosen at the start.
  pub filters: Vec<FileFilter>,
}

impl FileDialog {
  /// A dialog to choose a file to open.
  pub fn open(title: impl Into<String>) -> FileDialog {
    FileDialog {
      kind: FileDialogKind::Open,
      title: title.into(),
      folder: None,
      name: None,
      filters: Vec::new(),
    }
  }

  /// A dialog to choose a name to save as.
  pub fn save(title: impl Into<String>) -> FileDialog {
    FileDialog {
      kind: FileDialogKind::Save,
      ..FileDialog::open(title)
    }
  }

  /// Starts in `folder`.
  pub fn folder(
    mut self,
    folder: impl Into<PathBuf>,
  ) -> FileDialog {
    self.folder = Some(folder.into());
    self
  }

  /// Starts with `name`, for saving.
  pub fn name(
    mut self,
    name: impl Into<String>,
  ) -> FileDialog {
    self.name = Some(name.into());
    self
  }

  /// Offers files of this kind.
  pub fn filter(
    mut self,
    filter: FileFilter,
  ) -> FileDialog {
    self.filters.push(filter);
    self
  }

  /// The extension a saved name gets when it has none: the first filter's
  /// first extension.
  pub fn default_extension(&self) -> Option<&str> {
    self
      .filters
      .first()
      .and_then(|f| f.extensions.first())
      .map(String::as_str)
  }
}

/// Shows file dialogs.
pub trait FileDialogs {
  /// Shows `dialog`, and gives `done` the path chosen, or `None` when the
  /// user cancelled. Returns false, without calling `done`, when there is
  /// no dialog to show here; the caller can then show one of its own.
  ///
  /// `done` may be called before this returns, or later.
  fn show(
    &self,
    dialog: FileDialog,
    done: Callback<Option<PathBuf>>,
  ) -> bool;
}

thread_local! {
  static DIALOGS: RefCell<Option<Rc<dyn FileDialogs>>> = const { RefCell::new(None) };
}

/// Installs the file dialogs for this thread, or removes them with `None`.
/// The desktop crate installs the system's own; tests can install their
/// own.
pub fn set_file_dialogs(dialogs: Option<Rc<dyn FileDialogs>>) {
  DIALOGS.with_borrow_mut(|d| *d = dialogs);
}

/// The file dialogs installed now.
pub fn file_dialogs() -> Option<Rc<dyn FileDialogs>> {
  DIALOGS.with_borrow(Clone::clone)
}

/// Shows `dialog` with the installed file dialogs. Returns false, without
/// calling `done`, when there are none, or they cannot show it here.
pub fn show_file_dialog(
  dialog: FileDialog,
  done: Callback<Option<PathBuf>>,
) -> bool {
  // Cloned out first, so the dialogs may use this module themselves.
  match file_dialogs() {
    Some(d) => d.show(dialog, done),
    None => false,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;

  #[test]
  fn filters_match_extensions_without_case() {
    let f = FileFilter::new("Boards", &[".board", "brd"]);
    assert_eq!(f.extensions, ["board", "brd"]);
    assert!(f.accepts(Path::new("a/main.BOARD")));
    assert!(f.accepts(Path::new("x.brd")));
    assert!(!f.accepts(Path::new("notes.txt")));
    assert!(!f.accepts(Path::new("board")));
    assert!(FileFilter::new("All", &[]).accepts(Path::new("anything")));
  }

  #[test]
  fn without_dialogs_nothing_is_shown() {
    set_file_dialogs(None);
    let shown = show_file_dialog(
      FileDialog::save("Save").filter(FileFilter::new("Boards", &["board"])),
      Callback::new(|_| panic!("not called")),
    );
    assert!(!shown);
    assert_eq!(
      FileDialog::save("S")
        .filter(FileFilter::new("B", &["board"]))
        .default_extension(),
      Some("board")
    );
  }
}
