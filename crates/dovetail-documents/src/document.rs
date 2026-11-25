//! The document model: the data, its file, its undo history, whether it
//! has unsaved changes, and its snapshots for recovery.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use dovetail::{
  Callback, CloseGuard, FileDialog, FileFilter, Signal, Task, create_signal,
  show_file_dialog,
};
use dovetail_commands::{Commands, History};

use crate::recovery::{Recovered, Recovery, Snapshots};

/// Turns a document into bytes.
type ToBytes<T> = Rc<dyn Fn(&T) -> Vec<u8>>;
/// Reads a document from bytes, or says what is wrong with them.
type FromBytes<T> = Rc<dyn Fn(&[u8]) -> Result<T, String>>;

/// A kind of document: its name, its file extensions, how to make an empty
/// one, and how to turn it into bytes and back. The format is the
/// application's own.
pub struct DocumentKind<T> {
  name: String,
  extensions: Vec<String>,
  new: Rc<dyn Fn() -> T>,
  to_bytes: ToBytes<T>,
  from_bytes: FromBytes<T>,
}

impl<T> DocumentKind<T> {
  /// A kind called `name`, such as "Board", saved in files with these
  /// extensions (the first is the one new files get). `new` makes an empty
  /// document; `to_bytes` and `from_bytes` write and read the format, and
  /// `from_bytes` says what is wrong with a file it cannot read.
  pub fn new(
    name: impl Into<String>,
    extensions: &[&str],
    new: impl Fn() -> T + 'static,
    to_bytes: impl Fn(&T) -> Vec<u8> + 'static,
    from_bytes: impl Fn(&[u8]) -> Result<T, String> + 'static,
  ) -> DocumentKind<T> {
    DocumentKind {
      name: name.into(),
      extensions: extensions
        .iter()
        .map(|e| e.trim_start_matches('.').to_owned())
        .collect(),
      new: Rc::new(new),
      to_bytes: Rc::new(to_bytes),
      from_bytes: Rc::new(from_bytes),
    }
  }
}

/// What to do once a question about unsaved changes is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
  /// Start a new, empty document.
  New,
  /// Choose a file and open it.
  Open,
  /// Close the window.
  Close,
}

/// A question the document's frame shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Question {
  /// There are unsaved changes: save them first, throw them away, or
  /// cancel what was asked.
  Unsaved(After),
  /// Saving or opening failed, for this reason.
  Failed(String),
  /// Work left by a program that stopped can be recovered.
  Recover(Vec<Recovered>),
  /// No system file dialog is available here, so the frame shows its own.
  ChooseFile(FileDialog, Callback<Option<PathBuf>>),
}

/// The type-free parts of a document, which views and commands work with.
struct Ops {
  bytes: Rc<dyn Fn() -> Vec<u8>>,
  load: FromBytes<()>,
  reset: Rc<dyn Fn()>,
}

/// The file side of a document, without its data's type.
pub struct FileState {
  kind: String,
  extensions: Vec<String>,
  path: Option<PathBuf>,
  /// Names the document's state now: each edit gives a new one, and undo
  /// gives back the one before.
  version: u64,
  /// The version last saved or opened.
  saved: u64,
  next: u64,
  ops: Ops,
  history: History,
  guard: CloseGuard,
  question: Option<Question>,
  recovery: Option<Recovery>,
  snapshots: Option<Snapshots>,
}

impl std::fmt::Debug for FileState {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    write!(
      f,
      "FileState({:?}, {} / {})",
      self.path, self.version, self.saved
    )
  }
}

/// The file side of a [`Document`]: where it is saved, whether it has
/// unsaved changes, its undo history, and the questions its
/// [`DocumentFrame`](crate::DocumentFrame) asks. It is a small handle
/// that can be copied into closures, and does not depend on the data's
/// type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentFile(Signal<FileState>);

/// A document: data of type `T`, kept outside every widget, and changed
/// only with [`Document::edit`], which makes an undoable step. It is a
/// small handle that can be copied into closures.
///
/// Make it while a component builds: `use_state(|| Document::new(kind)).get()`.
pub struct Document<T: 'static> {
  data: Signal<T>,
  file: DocumentFile,
}

impl<T> Clone for Document<T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for Document<T> {}

impl<T> PartialEq for Document<T> {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    self.data == other.data && self.file == other.file
  }
}

impl<T> std::fmt::Debug for Document<T> {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    write!(f, "Document({:?})", self.file)
  }
}

impl<T: Clone + 'static> Document<T> {
  /// A new, empty, unsaved document of this kind.
  ///
  /// # Panics
  ///
  /// If no component is building, as for all state.
  pub fn new(kind: DocumentKind<T>) -> Document<T> {
    let data = create_signal((kind.new)());
    let ops = {
      let (to_bytes, from_bytes, new) =
        (kind.to_bytes, kind.from_bytes, kind.new);
      Ops {
        bytes: Rc::new(move || data.map(|d| to_bytes(d))),
        load: Rc::new(move |bytes| {
          let value = from_bytes(bytes)?;
          data.set(value);
          Ok(())
        }),
        reset: Rc::new(move || data.set(new())),
      }
    };
    let file = DocumentFile(create_signal(FileState {
      kind: kind.name,
      extensions: kind.extensions,
      path: None,
      version: 0,
      saved: 0,
      next: 1,
      ops,
      history: History::new(),
      guard: CloseGuard::new(),
      question: None,
      recovery: None,
      snapshots: None,
    }));
    Document { data, file }
  }

  /// A copy of the data. While a component builds, it is built again when
  /// the data changes.
  pub fn get(self) -> T {
    self.data.get()
  }

  /// Reads the data without copying it.
  pub fn read<R>(
    self,
    read: impl FnOnce(&T) -> R,
  ) -> R {
    self.data.map(read)
  }

  /// Changes the data with `change`, as one step that Undo takes back.
  /// `name` says what it was, as in "Undo Add part".
  pub fn edit(
    self,
    name: impl Into<String>,
    change: impl FnOnce(&mut T),
  ) {
    let before = self.data.get();
    let mut after = before.clone();
    change(&mut after);
    let file = self.file;
    let (was, now) = file.0.map(|s| (s.version, s.next));
    file.0.update(|s| s.next += 1);
    let data = self.data;
    let history = file.history();
    history.apply(
      name,
      move || {
        data.set(after.clone());
        file.set_version(now);
      },
      move || {
        data.set(before.clone());
        file.set_version(was);
      },
    );
  }

  /// The file side: saving, opening, unsaved changes and recovery.
  pub fn file(self) -> DocumentFile {
    self.file
  }
}

/// Writes `bytes` to `path` through a new file next to it, which then
/// replaces the old one, so a failed save leaves the old file whole.
fn write_whole(
  path: &Path,
  bytes: &[u8],
) -> std::io::Result<()> {
  let mut name = path
    .file_name()
    .ok_or_else(|| {
      std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name")
    })?
    .to_owned();
  name.push(".saving");
  let fresh = path.with_file_name(name);
  let written =
    std::fs::write(&fresh, bytes).and_then(|()| std::fs::rename(&fresh, path));
  if written.is_err() {
    let _ = std::fs::remove_file(&fresh);
  }
  written
}

impl DocumentFile {
  /// True when there are changes that are not saved. Undoing back to what
  /// was saved counts as saved.
  pub fn is_dirty(self) -> bool {
    self.0.map(|s| s.version != s.saved)
  }

  /// The file it was last saved as or opened from.
  pub fn path(self) -> Option<PathBuf> {
    self.0.map(|s| s.path.clone())
  }

  /// The file's name, or "Untitled".
  pub fn name(self) -> String {
    self.0.map(|s| {
      s.path
        .as_ref()
        .and_then(|p| p.file_name())
        .map_or_else(
          || "Untitled".to_owned(),
          |n| n.to_string_lossy().into_owned(),
        )
    })
  }

  /// The window title: the file's name, with `*` while there are unsaved
  /// changes, and the application's name.
  pub fn title(
    self,
    app: &str,
  ) -> String {
    let mark = if self.is_dirty() { "*" } else { "" };
    format!("{}{mark} - {app}", self.name())
  }

  /// The undo history.
  pub fn history(self) -> History {
    self.0.map(|s| s.history)
  }

  /// The guard that asks before the document's window closes.
  pub fn close_guard(self) -> CloseGuard {
    self.0.map(|s| s.guard)
  }

  /// The question the frame shows now.
  pub fn question(self) -> Option<Question> {
    self.0.map(|s| s.question.clone())
  }

  /// Shows a question, or none.
  pub fn set_question(
    self,
    question: Option<Question>,
  ) {
    self.0.update(|s| s.question = question);
  }

  /// The kind of document, as in "Board".
  pub fn kind(self) -> String {
    self.0.map(|s| s.kind.clone())
  }

  /// The file filters for dialogs: this kind's files, then any file.
  pub fn filters(self) -> Vec<FileFilter> {
    self.0.map(|s| {
      let ext: Vec<&str> = s.extensions.iter().map(String::as_str).collect();
      vec![
        FileFilter::new(format!("{} files", s.kind), &ext),
        FileFilter::new("All files", &[]),
      ]
    })
  }

  fn new_version(self) -> u64 {
    let mut id = 0;
    self.0.update(|s| {
      id = s.next;
      s.next += 1;
    });
    id
  }

  fn set_version(
    self,
    version: u64,
  ) {
    self.0.update(|s| s.version = version);
    self.changed();
  }

  /// Saves to `path`, which then becomes the document's file. Nothing
  /// changes when it fails.
  pub fn save_to(
    self,
    path: &Path,
  ) -> Result<(), String> {
    let bytes = self.0.map(|s| Rc::clone(&s.ops.bytes));
    write_whole(path, &bytes())
      .map_err(|e| format!("{} could not be saved: {e}", path.display()))?;
    self.0.update(|s| {
      s.path = Some(path.to_owned());
      s.saved = s.version;
    });
    self.clear_snapshot();
    Ok(())
  }

  /// Opens `path` in place of the document, forgetting its undo history.
  /// Nothing changes when it fails.
  pub fn open_from(
    self,
    path: &Path,
  ) -> Result<(), String> {
    let bytes = std::fs::read(path)
      .map_err(|e| format!("{} could not be opened: {e}", path.display()))?;
    let load = self.0.map(|s| Rc::clone(&s.ops.load));
    load(&bytes)
      .map_err(|e| format!("{} could not be read: {e}", path.display()))?;
    self.became(Some(path.to_owned()));
    Ok(())
  }

  /// Starts a new, empty document in place of this one, without asking.
  pub fn new_document(self) {
    let reset = self.0.map(|s| Rc::clone(&s.ops.reset));
    reset();
    self.became(None);
  }

  /// After the data was replaced: the new file, a clean state and no
  /// history.
  fn became(
    self,
    path: Option<PathBuf>,
  ) {
    let version = self.new_version();
    self.0.update(|s| {
      s.path = path;
      s.version = version;
      s.saved = version;
    });
    self.history().clear();
    self.clear_snapshot();
  }

  /// Defines the file commands: New (`file.new`, Mod+N), Open
  /// (`file.open`, Mod+O), Save (`file.save`, Mod+S) and Save As
  /// (`file.save_as`, Mod+Shift+S), and Undo and Redo for the history.
  /// New and Open ask first when there are unsaved changes.
  pub fn add_commands(
    self,
    commands: Commands,
  ) -> Commands {
    let commands = commands
      .define("file.new", "New", Some("Mod+N"), move || {
        self.ask(After::New)
      })
      .define("file.open", "Open...", Some("Mod+O"), move || {
        self.ask(After::Open)
      })
      .define("file.save", "Save", Some("Mod+S"), move || self.save(None))
      .define(
        "file.save_as",
        "Save As...",
        Some("Mod+Shift+S"),
        move || self.save_as(None),
      );
    self.history().add_commands(commands)
  }

  /// Does `after` now, or asks first when there are unsaved changes.
  pub fn ask(
    self,
    after: After,
  ) {
    if self.is_dirty() {
      self.set_question(Some(Question::Unsaved(after)));
    } else {
      self.carry_out(after);
    }
  }

  /// Does `after`, without asking.
  pub fn carry_out(
    self,
    after: After,
  ) {
    self.set_question(None);
    match after {
      After::New => self.new_document(),
      After::Open => {
        let dialog = FileDialog::open(format!("Open {}", self.kind()));
        let dialog = self.with_filters(self.near_file(dialog));
        self.choose(dialog, move |path| {
          if let Err(e) = self.open_from(&path) {
            self.set_question(Some(Question::Failed(e)));
          }
        });
      },
      After::Close => {
        // Closed on purpose: the unsaved work is not kept for recovery.
        self.clear_snapshot();
        self.close_guard().close_now();
      },
    }
  }

  /// Saves to the document's file, or asks for one when it has none, and
  /// then does `after`.
  pub fn save(
    self,
    after: Option<After>,
  ) {
    match self.path() {
      Some(path) => self.save_and(&path, after),
      None => self.save_as(after),
    }
  }

  /// Asks for a file to save as, saves, and then does `after`.
  pub fn save_as(
    self,
    after: Option<After>,
  ) {
    let dialog = FileDialog::save(format!("Save {}", self.kind()));
    let mut dialog = self.with_filters(self.near_file(dialog));
    if dialog.name.is_none() {
      let ext = self.0.map(|s| s.extensions.first().cloned());
      dialog.name = Some(
        ext.map_or_else(|| "Untitled".into(), |e| format!("Untitled.{e}")),
      );
    }
    self.choose(dialog, move |path| self.save_and(&path, after));
  }

  fn save_and(
    self,
    path: &Path,
    after: Option<After>,
  ) {
    match self.save_to(path) {
      Ok(()) => match after {
        Some(after) => self.carry_out(after),
        None => self.set_question(None),
      },
      Err(e) => self.set_question(Some(Question::Failed(e))),
    }
  }

  fn with_filters(
    self,
    mut dialog: FileDialog,
  ) -> FileDialog {
    dialog.filters = self.filters();
    dialog
  }

  /// Starts the dialog in the document's folder, with its name.
  fn near_file(
    self,
    mut dialog: FileDialog,
  ) -> FileDialog {
    if let Some(path) = self.path() {
      dialog.folder = path.parent().map(Path::to_owned);
      if dialog.kind == dovetail::FileDialogKind::Save {
        dialog.name = path
          .file_name()
          .map(|n| n.to_string_lossy().into_owned());
      }
    }
    dialog
  }

  /// Shows a file dialog, the system's or the frame's own, and gives
  /// `chosen` the path unless the user cancels.
  fn choose(
    self,
    dialog: FileDialog,
    chosen: impl Fn(PathBuf) + 'static,
  ) {
    let done = Callback::new(move |path: Option<PathBuf>| {
      if let Some(path) = path {
        chosen(path);
      }
    });
    self.set_question(None);
    if !show_file_dialog(dialog.clone(), done.clone()) {
      self.set_question(Some(Question::ChooseFile(dialog, done)));
    }
  }

  /// Keeps unsaved work in `recovery` from now on, and offers back what a
  /// program that stopped left there for this kind of document.
  pub fn keep_recovery(
    self,
    recovery: Recovery,
  ) {
    let kind = self.kind();
    let found: Vec<Recovered> = recovery
      .found()
      .into_iter()
      .filter(|r| r.kind == kind)
      .collect();
    self.0.update(|s| {
      s.recovery = Some(recovery);
      if !found.is_empty() {
        s.question = Some(Question::Recover(found));
      }
    });
  }

  /// Takes back the newest recovered work: the document then holds it,
  /// with unsaved changes, and the snapshots it came from are removed.
  pub fn recover(
    self,
    found: &[Recovered],
  ) -> Result<(), String> {
    let Some(newest) = found.first() else {
      return Ok(());
    };
    let load = self.0.map(|s| Rc::clone(&s.ops.load));
    load(&newest.bytes)
      .map_err(|e| format!("The work could not be recovered: {e}"))?;
    self.became(newest.original.clone());
    // Unsaved: the work is newer than the file, if there is one.
    let version = self.new_version();
    self.set_version(version);
    self.write_snapshot();
    for r in found {
      r.discard();
    }
    self.set_question(None);
    Ok(())
  }

  /// Something changed: snapshots follow. A document without unsaved
  /// changes has none.
  fn changed(self) {
    if self.0.map(|s| s.recovery.is_none()) {
      return;
    }
    if !self.is_dirty() {
      self.clear_snapshot();
      return;
    }
    let now = Instant::now();
    let mut write_now = false;
    let mut idle = None;
    let mut old_timer = None;
    self.0.update(|s| {
      let Some(recovery) = s.recovery.clone() else {
        return;
      };
      if s.snapshots.is_none() {
        match Snapshots::start(&recovery) {
          Ok(snapshots) => s.snapshots = Some(snapshots),
          Err(e) => {
            eprintln!("documents: unsaved work cannot be kept: {e}");
            return;
          },
        }
      }
      let snaps = s.snapshots.as_mut().expect("just made");
      let since = *snaps.pending_since.get_or_insert(now);
      old_timer = snaps.timer.take();
      if now.duration_since(since) >= recovery.longest() {
        write_now = true;
      } else {
        idle = Some(recovery.idle());
      }
    });
    if let Some(t) = old_timer {
      t.cancel();
    }
    if write_now {
      self.write_snapshot();
    } else if let Some(idle) = idle {
      let timer =
        Task::after(idle, Callback::new(move |()| self.write_snapshot()));
      self.0.update(|s| {
        if let Some(snaps) = s.snapshots.as_mut() {
          snaps.timer = Some(timer);
        }
      });
    }
  }

  /// Writes a snapshot now, if there are unsaved changes.
  pub fn write_snapshot(self) {
    if !self.is_dirty() {
      return;
    }
    let (bytes, kind, path) = self
      .0
      .map(|s| (Rc::clone(&s.ops.bytes), s.kind.clone(), s.path.clone()));
    let bytes = bytes();
    self.0.update(|s| {
      if let Some(snaps) = s.snapshots.as_mut()
        && let Err(e) = snaps.write(&kind, path.as_deref(), &bytes)
      {
        eprintln!("documents: unsaved work could not be kept: {e}");
      }
    });
  }

  /// The snapshot file, while there is one.
  pub fn snapshot_file(self) -> Option<PathBuf> {
    self.0.map(|s| {
      s.snapshots
        .as_ref()
        .map(Snapshots::file)
        .filter(|f| f.exists())
    })
  }

  fn clear_snapshot(self) {
    self.0.update(|s| {
      if let Some(snaps) = s.snapshots.as_mut() {
        snaps.clear();
      }
    });
  }
}
