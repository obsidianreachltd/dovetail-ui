//! The document model without a window: edits and undo, unsaved changes,
//! saving and opening, the file commands and questions, and recovery.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use dovetail_commands::Commands;
use dovetail_core::{
  Callback, FileDialog, FileDialogs, run_task_messages, set_file_dialogs,
  take_close_requests,
};
use dovetail_documents::{
  After, Document, DocumentKind, Question, Recovery, owned,
};

type List = Vec<String>;

/// A list of lines, saved one per line. A line starting with `!` cannot
/// be read.
fn kind() -> DocumentKind<List> {
  DocumentKind::new(
    "List",
    &["list"],
    Vec::new,
    |l: &List| l.join("\n").into_bytes(),
    |bytes| {
      let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
      if text.starts_with('!') {
        return Err("not a list".into());
      }
      Ok(text.lines().map(str::to_owned).collect())
    },
  )
}

fn folder(name: &str) -> PathBuf {
  let dir = std::env::temp_dir().join(format!(
    "dovetail-documents-test-{name}-{}",
    std::process::id()
  ));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  dir
}

fn add(
  doc: Document<List>,
  item: &str,
) {
  let item = item.to_owned();
  doc.edit(format!("Add {item}"), move |l| l.push(item));
}

#[test]
fn edits_undo_and_unsaved_changes() {
  let (_tree, doc) = owned(|| Document::new(kind()));
  let file = doc.file();
  assert!(!file.is_dirty());
  assert_eq!(file.title("Lists"), "Untitled - Lists");
  add(doc, "milk");
  add(doc, "eggs");
  assert_eq!(doc.get(), ["milk", "eggs"]);
  assert!(file.is_dirty());
  assert_eq!(file.title("Lists"), "Untitled* - Lists");
  let history = file.history();
  assert_eq!(history.undo_name().as_deref(), Some("Add eggs"));
  history.undo();
  history.undo();
  assert!(doc.get().is_empty());
  assert!(!file.is_dirty(), "undone back to the start is unchanged");
  history.redo();
  assert!(file.is_dirty());

  let dir = folder("undo");
  let path = dir.join("shop.list");
  file.save_to(&path).unwrap();
  assert!(!file.is_dirty());
  assert_eq!(file.title("Lists"), "shop.list - Lists");
  history.undo();
  assert!(file.is_dirty(), "undone past the save");
  history.redo();
  assert!(!file.is_dirty(), "redone back to what was saved");
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_and_opening_give_the_same_document() {
  let dir = folder("round-trip");
  let path = dir.join("a.list");
  let (_a, first) = owned(|| Document::new(kind()));
  for item in ["one", "two", "three \u{2713}"] {
    add(first, item);
  }
  first.file().save_to(&path).unwrap();
  assert_eq!(first.file().path().as_deref(), Some(path.as_path()));

  let (_b, second) = owned(|| Document::new(kind()));
  add(second, "something else");
  second.file().open_from(&path).unwrap();
  assert_eq!(second.get(), first.get());
  assert!(!second.file().is_dirty());
  assert!(!second.file().history().can_undo(), "a new history");
  assert_eq!(second.file().name(), "a.list");
  // No file is left over from saving.
  let names: Vec<_> = std::fs::read_dir(&dir)
    .unwrap()
    .flatten()
    .map(|e| e.file_name())
    .collect();
  assert_eq!(names, ["a.list"]);
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failures_change_nothing() {
  let dir = folder("failures");
  let (_tree, doc) = owned(|| Document::new(kind()));
  add(doc, "kept");
  let missing = dir.join("no such folder").join("x.list");
  let e = doc.file().save_to(&missing).unwrap_err();
  assert!(e.contains("could not be saved"), "{e}");
  assert!(doc.file().is_dirty());
  assert_eq!(doc.file().path(), None);

  let bad = dir.join("bad.list");
  std::fs::write(&bad, "!broken").unwrap();
  let e = doc.file().open_from(&bad).unwrap_err();
  assert!(e.contains("could not be read: not a list"), "{e}");
  assert_eq!(doc.get(), ["kept"]);
  assert!(doc.file().is_dirty());
  assert!(
    doc
      .file()
      .open_from(&dir.join("gone.list"))
      .is_err()
  );
  let _ = std::fs::remove_dir_all(&dir);
}

/// File dialogs that answer with the paths given, in turn.
struct Answers(RefCell<Vec<Option<PathBuf>>>, RefCell<Vec<FileDialog>>);

impl FileDialogs for Answers {
  fn show(
    &self,
    dialog: FileDialog,
    done: Callback<Option<PathBuf>>,
  ) -> bool {
    self.1.borrow_mut().push(dialog);
    let answer = self.0.borrow_mut().remove(0);
    done.call(answer);
    true
  }
}

#[test]
fn file_commands_ask_about_unsaved_changes() {
  let dir = folder("commands");
  let (_tree, (doc, commands)) = owned(|| {
    let doc = Document::new(kind());
    let commands = doc.file().add_commands(Commands::new());
    (doc, commands)
  });
  let file = doc.file();
  let saved = dir.join("saved.list");
  let answers = Rc::new(Answers(
    RefCell::new(vec![
      None,
      Some(saved.clone()),
      Some(saved.clone()),
    ]),
    RefCell::new(Vec::new()),
  ));
  set_file_dialogs(Some(answers.clone()));

  add(doc, "a");
  assert!(commands.run("edit.undo"));
  assert!(doc.get().is_empty());
  assert!(commands.run("edit.redo"));

  // Save with no file asks for one; cancelled, nothing happens.
  assert!(commands.run("file.save"));
  assert!(file.is_dirty());
  assert!(!saved.exists());
  // Asked again, it saves.
  assert!(commands.run("file.save"));
  assert!(!file.is_dirty());
  assert!(saved.exists());
  {
    let shown = answers.1.borrow();
    assert_eq!(shown[1].name.as_deref(), Some("Untitled.list"));
    assert_eq!(shown[1].default_extension(), Some("list"));
  }

  // New with unsaved changes asks first; Don't save starts afresh.
  add(doc, "b");
  assert!(commands.run("file.new"));
  assert_eq!(file.question(), Some(Question::Unsaved(After::New)));
  assert_eq!(doc.get(), ["a", "b"], "nothing changes before the answer");
  file.carry_out(After::New);
  assert!(doc.get().is_empty());
  assert_eq!(file.path(), None);
  assert_eq!(file.question(), None);

  // Open without unsaved changes opens at once.
  assert!(commands.run("file.open"));
  assert_eq!(doc.get(), ["a"]);
  assert_eq!(file.path().as_deref(), Some(saved.as_path()));
  {
    let shown = answers.1.borrow();
    // After New there is no file, so no folder to start in.
    assert_eq!(shown[2].kind, dovetail_core::FileDialogKind::Open);
    assert_eq!(shown[2].folder, None);
    assert_eq!(shown[2].filters[0].extensions, ["list"]);
  }
  set_file_dialogs(None);
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_system_dialogs_the_frame_shows_its_own() {
  let dir = folder("own-dialog");
  set_file_dialogs(None);
  let (_tree, doc) = owned(|| Document::new(kind()));
  let file = doc.file();
  add(doc, "x");
  file.save(None);
  let Some(Question::ChooseFile(dialog, done)) = file.question() else {
    panic!("the frame's own dialog: {:?}", file.question());
  };
  assert_eq!(dialog.kind, dovetail_core::FileDialogKind::Save);
  let path = dir.join("own.list");
  done.call(Some(path.clone()));
  assert!(path.exists());
  assert!(!file.is_dirty());
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_asks_only_with_unsaved_changes() {
  let (_tree, doc) = owned(|| Document::new(kind()));
  let file = doc.file();
  take_close_requests();
  file.ask(After::Close);
  assert_eq!(take_close_requests(), [file.close_guard()]);
  add(doc, "x");
  file.ask(After::Close);
  assert!(take_close_requests().is_empty());
  assert_eq!(file.question(), Some(Question::Unsaved(After::Close)));
  file.carry_out(After::Close);
  assert_eq!(take_close_requests(), [file.close_guard()]);
}

fn wait_until(
  what: &str,
  done: impl Fn() -> bool,
) {
  let start = Instant::now();
  while !done() {
    assert!(
      start.elapsed() < Duration::from_secs(10),
      "waited for {what}"
    );
    run_task_messages();
    std::thread::sleep(Duration::from_millis(5));
  }
}

#[test]
fn unsaved_work_is_recovered_after_the_program_stops() {
  let dir = folder("recovery");
  let recovery = Recovery::new(dir.join("recovery"))
    .with_delays(Duration::from_millis(30), Duration::from_secs(5));
  let original = dir.join("work.list");
  let (tree, doc) = {
    let recovery = recovery.clone();
    owned(move || {
      let doc = Document::new(kind());
      doc.file().keep_recovery(recovery);
      doc
    })
  };
  add(doc, "saved line");
  doc.file().save_to(&original).unwrap();
  assert_eq!(doc.file().snapshot_file(), None, "nothing unsaved yet");
  add(doc, "unsaved line");
  wait_until("a snapshot", || doc.file().snapshot_file().is_some());
  assert!(recovery.found().is_empty(), "held while the program runs");

  // Undo back to what was saved: the snapshot goes.
  doc.file().history().undo();
  assert_eq!(doc.file().snapshot_file(), None);
  doc.file().history().redo();
  wait_until("a snapshot again", || doc.file().snapshot_file().is_some());

  // The program stops: its state goes without saving.
  drop(tree);
  let found = recovery.found();
  assert_eq!(found.len(), 1);
  assert_eq!(found[0].original.as_deref(), Some(original.as_path()));

  let (_tree, again) = {
    let recovery = recovery.clone();
    owned(move || {
      let doc = Document::new(kind());
      doc.file().keep_recovery(recovery);
      doc
    })
  };
  let Some(Question::Recover(offered)) = again.file().question() else {
    panic!("recovery offered: {:?}", again.file().question());
  };
  again.file().recover(&offered).unwrap();
  assert_eq!(again.get(), ["saved line", "unsaved line"]);
  assert!(again.file().is_dirty(), "recovered work is not saved yet");
  assert_eq!(again.file().path().as_deref(), Some(original.as_path()));
  assert_eq!(again.file().question(), None);
  assert!(
    again.file().snapshot_file().is_some(),
    "the recovered work is kept again at once"
  );
  assert!(!offered[0].file.exists(), "the old snapshot is gone");

  // Saving removes the snapshot; nothing is offered after that.
  again.file().save_to(&original).unwrap();
  assert_eq!(again.file().snapshot_file(), None);
  assert!(recovery.found().is_empty());
  assert_eq!(
    std::fs::read_to_string(&original).unwrap(),
    "saved line\nunsaved line"
  );
  let _ = Path::new(&dir);
  let _ = std::fs::remove_dir_all(&dir);
}
