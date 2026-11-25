//! File dialogs on Linux and other Unix systems (M11, D0017): `zenity` or
//! `kdialog`, run as programs on a task's thread, so the window keeps
//! drawing while the dialog is open. The arguments are built on every
//! system, so they are tested everywhere.

// Used only on Linux and other Unix systems, and in tests.
#![cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]

use std::path::{Path, PathBuf};

use dovetail_core::{FileDialog, FileDialogKind, FileFilter};

/// A program that shows file dialogs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Program {
  Zenity,
  Kdialog,
}

impl Program {
  fn name(self) -> &'static str {
    match self {
      Program::Zenity => "zenity",
      Program::Kdialog => "kdialog",
    }
  }
}

/// Where the dialog starts: the folder and the name, joined.
fn start(dialog: &FileDialog) -> Option<PathBuf> {
  match (&dialog.folder, &dialog.name) {
    (Some(f), Some(n)) => Some(f.join(n)),
    (Some(f), None) => Some(f.clone()),
    (None, Some(n)) => Some(PathBuf::from(n)),
    (None, None) => None,
  }
}

fn patterns(f: &FileFilter) -> String {
  if f.extensions.is_empty() {
    "*".to_owned()
  } else {
    f.extensions
      .iter()
      .map(|e| format!("*.{e}"))
      .collect::<Vec<_>>()
      .join(" ")
  }
}

/// The arguments that show `dialog` with `program`.
pub(crate) fn arguments(
  program: Program,
  dialog: &FileDialog,
) -> Vec<String> {
  let save = dialog.kind == FileDialogKind::Save;
  let mut args = Vec::new();
  match program {
    Program::Zenity => {
      args.push("--file-selection".to_owned());
      args.push(format!("--title={}", dialog.title));
      if save {
        args.push("--save".to_owned());
        args.push("--confirm-overwrite".to_owned());
      }
      if let Some(start) = start(dialog) {
        let mut s = start.display().to_string();
        // zenity opens a folder only when its name ends in a slash.
        if dialog.name.is_none() && !s.ends_with('/') {
          s.push('/');
        }
        args.push(format!("--filename={s}"));
      }
      for f in &dialog.filters {
        args.push(format!("--file-filter={} | {}", f.name, patterns(f)));
      }
    },
    Program::Kdialog => {
      args.push("--title".to_owned());
      args.push(dialog.title.clone());
      args.push(
        if save {
          "--getsavefilename"
        } else {
          "--getopenfilename"
        }
        .to_owned(),
      );
      args.push(
        start(dialog)
          .map_or_else(|| ".".to_owned(), |s| s.display().to_string()),
      );
      if !dialog.filters.is_empty() {
        args.push(
          dialog
            .filters
            .iter()
            .map(|f| format!("{}|{}", patterns(f), f.name))
            .collect::<Vec<_>>()
            .join("\n"),
        );
      }
    },
  }
  args
}

/// The path a program printed, without the line break.
pub(crate) fn chosen(output: &str) -> Option<PathBuf> {
  let line = output.trim_end_matches(['\n', '\r']);
  (!line.is_empty()).then(|| PathBuf::from(line))
}

/// The first of zenity and kdialog found on the `PATH`.
pub(crate) fn find_program() -> Option<Program> {
  let path = std::env::var_os("PATH")?;
  let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
  let found = |name: &str| dirs.iter().any(|d| d.join(name).is_file());
  // kdialog first on KDE, whose dialogs it shows.
  let kde = std::env::var("XDG_CURRENT_DESKTOP")
    .is_ok_and(|d| d.to_ascii_uppercase().contains("KDE"));
  let order = if kde {
    [Program::Kdialog, Program::Zenity]
  } else {
    [Program::Zenity, Program::Kdialog]
  };
  order.into_iter().find(|p| found(p.name()))
}

/// Shows file dialogs by running `zenity` or `kdialog`.
pub(crate) struct ProgramDialogs(pub(crate) Program);

impl dovetail_core::FileDialogs for ProgramDialogs {
  fn show(
    &self,
    dialog: FileDialog,
    done: dovetail_core::Callback<Option<PathBuf>>,
  ) -> bool {
    let program = self.0;
    let args = arguments(program, &dialog);
    let extension = dialog.default_extension().map(str::to_owned);
    let save = dialog.kind == FileDialogKind::Save;
    dovetail_core::spawn_task(
      move |_| {
        let output = std::process::Command::new(program.name())
          .args(&args)
          .output()
          .ok()?;
        // A cancelled dialog exits with 1 and prints nothing.
        let path = chosen(&String::from_utf8_lossy(&output.stdout))?;
        Some(with_extension(path, save, extension.as_deref()))
      },
      None,
      dovetail_core::Callback::new(move |end| {
        done.call(match end {
          dovetail_core::TaskEnd::Done(path) => Some(path),
          _ => None,
        })
      }),
    );
    true
  }
}

/// A saved name without an extension gets the first filter's.
fn with_extension(
  path: PathBuf,
  save: bool,
  extension: Option<&str>,
) -> PathBuf {
  match extension {
    Some(e) if save && Path::new(&path).extension().is_none() => {
      path.with_extension(e)
    },
    _ => path,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn dialog() -> FileDialog {
    FileDialog::save("Save board")
      .folder("/home/a/boards")
      .name("main.board")
      .filter(FileFilter::new("Board files", &["board", "brd"]))
      .filter(FileFilter::new("All files", &[]))
  }

  #[test]
  fn zenity_arguments() {
    assert_eq!(
      arguments(Program::Zenity, &dialog()),
      [
        "--file-selection",
        "--title=Save board",
        "--save",
        "--confirm-overwrite",
        &format!(
          "--filename={}",
          Path::new("/home/a/boards")
            .join("main.board")
            .display()
        ),
        "--file-filter=Board files | *.board *.brd",
        "--file-filter=All files | *",
      ]
    );
    let open = FileDialog::open("Open").folder("/tmp");
    let args = arguments(Program::Zenity, &open);
    assert!(
      args.last().unwrap().ends_with('/'),
      "a folder ends in a slash: {args:?}"
    );
  }

  #[test]
  fn kdialog_arguments() {
    let args = arguments(Program::Kdialog, &dialog());
    assert_eq!(
      &args[..3],
      [
        "--title",
        "Save board",
        "--getsavefilename"
      ]
    );
    assert_eq!(args[4], "*.board *.brd|Board files\n*|All files");
    assert_eq!(arguments(Program::Kdialog, &FileDialog::open("O"))[3], ".");
  }

  #[test]
  fn output_and_extensions() {
    assert_eq!(chosen("/a/b.board\n"), Some(PathBuf::from("/a/b.board")));
    assert_eq!(chosen("\n"), None);
    assert_eq!(
      with_extension(PathBuf::from("/a/b"), true, Some("board")),
      PathBuf::from("/a/b.board")
    );
    assert_eq!(
      with_extension(PathBuf::from("/a/b.txt"), true, Some("board")),
      PathBuf::from("/a/b.txt")
    );
    assert_eq!(
      with_extension(PathBuf::from("/a/b"), false, Some("board")),
      PathBuf::from("/a/b")
    );
  }
}
