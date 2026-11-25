//! The views: the frame round a document's window, the questions it asks,
//! its own file dialog, and a box that shows a task's progress.

use std::path::{Path, PathBuf};

use dovetail::{
  Align, Border, Callback, Children, Edges, Element, ElementKind, FileDialog,
  FileDialogKind, Progress, Role, Size, Style, View, use_state,
};
use dovetail_controls::{Dialog, DialogButton, ProgressBar, TextField};
use dovetail_macros::{component, ui};

use crate::document::{After, DocumentFile, Question};

/// The frame round a document's window. It fills the window with its
/// children, sets the window's title (the file's name, with `*` while
/// there are unsaved changes, and `app`), asks before the window closes
/// with unsaved changes, and shows the document's questions.
#[component]
pub fn DocumentFrame(
  file: DocumentFile,
  #[prop(into)] app: String,
  children: Children,
) -> View {
  let body: View = Element::new(ElementKind::Column)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      align: Align::Stretch,
      ..Style::default()
    })
    .children(children)
    .into();
  let mut parts = vec![body];
  if let Some(question) = file.question() {
    parts.push(question_view(file, question));
  }
  Element::new(ElementKind::Stack)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      align: Align::Stretch,
      ..Style::default()
    })
    .window_title(file.title(&app))
    .close_guard(
      file.close_guard(),
      Callback::new(move |()| file.ask(After::Close)),
    )
    .children(parts)
    .into()
}

fn question_view(
  file: DocumentFile,
  question: Question,
) -> View {
  let cancel = move |()| file.set_question(None);
  match question {
    Question::Unsaved(after) => {
      let doing = match after {
        After::New => "starting a new one",
        After::Open => "opening another",
        After::Close => "closing",
      };
      ui! {
        Dialog {
          title: "Unsaved changes",
          message: format!("Save the changes to {} before {doing}?", file.name()),
          buttons: vec![
            DialogButton::new("Save", move || file.save(Some(after))),
            DialogButton::new("Don't save", move || file.carry_out(after)),
            DialogButton::new("Cancel", move || file.set_question(None)),
          ],
          on_cancel: cancel,
        }
      }
    },
    Question::Failed(message) => ui! {
      Dialog {
        title: "Something went wrong",
        message: message,
        buttons: vec![DialogButton::new("OK", move || file.set_question(None))],
        on_cancel: cancel,
      }
    },
    Question::Recover(found) => {
      let newest = &found[0];
      let when = ago(newest.written);
      let message = format!(
        "{} was not saved when the program stopped {when}. Recover the work?",
        newest.name()
      );
      let (keep, drop) = (found.clone(), found);
      ui! {
        Dialog {
          title: "Recover unsaved work",
          message: message,
          buttons: vec![
            DialogButton::new("Recover", move || {
              if let Err(e) = file.recover(&keep) {
                file.set_question(Some(Question::Failed(e)));
              }
            }),
            DialogButton::new("Discard", move || {
              for r in &drop {
                r.discard();
              }
              file.set_question(None);
            }),
          ],
          on_cancel: cancel,
        }
      }
    },
    Question::ChooseFile(dialog, done) => {
      ui! { FilePicker { file: file, dialog: dialog, done: move |p| done.call(p) } }
    },
  }
}

/// How long ago `time` was, in words.
fn ago(time: std::time::SystemTime) -> String {
  let secs = time.elapsed().map_or(0, |d| d.as_secs());
  match secs {
    0..60 => "less than a minute ago".into(),
    60..120 => "a minute ago".into(),
    120..3600 => format!("{} minutes ago", secs / 60),
    3600..7200 => "an hour ago".into(),
    _ => format!("{} hours ago", secs / 3600),
  }
}

/// Where the picker starts: the dialog's folder and name, or the current
/// folder.
fn start_path(dialog: &FileDialog) -> PathBuf {
  let folder = dialog
    .folder
    .clone()
    .or_else(|| std::env::current_dir().ok())
    .unwrap_or_default();
  match &dialog.name {
    Some(name) => folder.join(name),
    None => folder.join(""),
  }
}

/// The files in `folder` that the dialog's first filter takes, by name.
fn files_in(
  folder: &Path,
  dialog: &FileDialog,
) -> Vec<PathBuf> {
  let Ok(entries) = std::fs::read_dir(folder) else {
    return Vec::new();
  };
  let filter = dialog.filters.first();
  let mut files: Vec<PathBuf> = entries
    .flatten()
    .map(|e| e.path())
    .filter(|p| p.is_file() && filter.is_none_or(|f| f.accepts(p)))
    .collect();
  files.sort();
  files.truncate(12);
  files
}

/// The file dialog drawn by Dovetail, for systems with none of their own
/// (D0017): a path to type, and the files in its folder to pick from.
#[component]
fn FilePicker(
  file: DocumentFile,
  dialog: FileDialog,
  done: Callback<Option<PathBuf>>,
) -> View {
  let path = use_state({
    let dialog = dialog.clone();
    move || start_path(&dialog).display().to_string()
  });
  let typed = PathBuf::from(path.get());
  let folder = if typed.is_dir() {
    typed.clone()
  } else {
    typed
      .parent()
      .map(Path::to_owned)
      .unwrap_or_default()
  };
  let rows: Vec<View> = files_in(&folder, &dialog)
    .into_iter()
    .map(|p| {
      let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
      let shown = p.display().to_string();
      Element::new(ElementKind::Box)
        .style(Style {
          padding: Edges::xy(6.0, 3.0),
          radius: 4.0,
          ..Style::default()
        })
        .role(Role::ListBoxOption)
        .focusable(true)
        .on_press(Callback::new(move |()| path.set(shown.clone())))
        .children([View::text(name)])
        .into()
    })
    .collect();
  let list: View = Element::new(ElementKind::Column)
    .style(Style {
      gap: 2.0,
      padding: Edges::all(4.0),
      align: Align::Stretch,
      border: Border {
        width: 1.0,
        color: dovetail_theme::theme().colors.outline_variant,
      },
      radius: 4.0,
      min_height: 60.0,
      ..Style::default()
    })
    .role(Role::ListBox)
    .label(format!("Files in {}", folder.display()))
    .children(if rows.is_empty() {
      vec![View::text("No files here.")]
    } else {
      rows
    })
    .into();
  let field = ui! {
    TextField {
      value: path.get(),
      on_change: move |v| path.set(v),
      width: 440.0,
      label: "File",
    }
  };
  let content: View = Element::new(ElementKind::Column)
    .style(Style {
      gap: 8.0,
      align: Align::Stretch,
      ..Style::default()
    })
    .children([field, list])
    .into();
  let save = dialog.kind == FileDialogKind::Save;
  let ext = dialog.default_extension().map(str::to_owned);
  let choose = {
    let done = done.clone();
    move || {
      let mut chosen = PathBuf::from(path.get());
      if chosen.as_os_str().is_empty() || chosen.is_dir() {
        return;
      }
      if save
        && chosen.extension().is_none()
        && let Some(ext) = &ext
      {
        chosen.set_extension(ext);
      }
      if !save && !chosen.is_file() {
        return;
      }
      file.set_question(None);
      done.call(Some(chosen));
    }
  };
  let cancel = move || {
    file.set_question(None);
    done.call(None);
  };
  let cancel_key = cancel.clone();
  ui! {
    Dialog {
      title: dialog.title.clone(),
      message: "",
      content: Some(content),
      buttons: vec![
        DialogButton::new(if save { "Save" } else { "Open" }, choose),
        DialogButton::new("Cancel", cancel),
      ],
      on_cancel: move |()| cancel_key(),
    }
  }
}

/// A box above the window that shows a task's progress, with a Cancel
/// button that calls `on_cancel`. Show it while the task runs.
#[component]
pub fn TaskProgress(
  #[prop(into)] title: String,
  #[prop(default)] progress: Option<Progress>,
  on_cancel: Callback<()>,
) -> View {
  let progress = progress.unwrap_or_default();
  let percent = progress
    .fraction
    .map_or_else(String::new, |f| format!("{:.0}%", f * 100.0));
  let line = match (progress.message.is_empty(), percent.is_empty()) {
    (true, _) => percent,
    (false, true) => progress.message.clone(),
    (false, false) => format!("{} ({percent})", progress.message),
  };
  let content: View = Element::new(ElementKind::Column)
    .style(Style { gap: 8.0, ..Style::default() })
    .children([
      ui! { ProgressBar { value: progress.fraction, width: 320.0, label: title.clone() } },
      View::text(line),
    ])
    .into();
  let cancel = on_cancel.clone();
  ui! {
    Dialog {
      title: title,
      content: Some(content),
      buttons: vec![DialogButton::new("Cancel", move || cancel.call(()))],
      on_cancel: move |()| on_cancel.call(()),
    }
  }
}
