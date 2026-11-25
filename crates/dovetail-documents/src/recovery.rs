//! Recovery after a crash (D0017): snapshots of unsaved work, in a folder
//! the application chooses.
//!
//! Each document with unsaved changes has two files there, named alike:
//!
//! - `NAME.recover`, the latest snapshot: a few lines that say what it is,
//!   a blank line, and the document's bytes, as saving would write them;
//! - `NAME.lock`, which the running program keeps locked, so that another
//!   copy of the program does not offer its work back while it runs.
//!
//! A snapshot whose lock nobody holds was left by a program that stopped
//! without saving, and is offered back.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const HEADER: &str = "dovetail-recovery 1";

/// The folder where unsaved work is kept, and how often it is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
  dir: PathBuf,
  idle: Duration,
  longest: Duration,
}

impl Recovery {
  /// Keeps unsaved work in `dir`, such as a `recovery` folder in the
  /// application's settings folder. A snapshot is written 2 seconds after
  /// edits stop, and at least every 30 seconds while edits go on.
  pub fn new(dir: impl Into<PathBuf>) -> Recovery {
    Recovery {
      dir: dir.into(),
      idle: Duration::from_secs(2),
      longest: Duration::from_secs(30),
    }
  }

  /// Writes a snapshot `idle` after edits stop, and at least every
  /// `longest` while edits go on.
  pub fn with_delays(
    mut self,
    idle: Duration,
    longest: Duration,
  ) -> Recovery {
    self.idle = idle;
    self.longest = longest;
    self
  }

  /// The folder.
  pub fn dir(&self) -> &Path {
    &self.dir
  }

  pub(crate) fn idle(&self) -> Duration {
    self.idle
  }

  pub(crate) fn longest(&self) -> Duration {
    self.longest
  }

  /// The unsaved work left by programs that stopped, newest first. Work
  /// that a running program holds is left out, and so is a file that
  /// cannot be read.
  pub fn found(&self) -> Vec<Recovered> {
    let Ok(entries) = std::fs::read_dir(&self.dir) else {
      return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
      let path = entry.path();
      let ext = path.extension().and_then(|e| e.to_str());
      let lock = path.with_extension("lock");
      match ext {
        Some("recover") => {
          let Some(_held) = take_lock(&lock) else {
            continue;
          };
          if let Ok(bytes) = std::fs::read(&path)
            && let Some(r) = parse(&path, &bytes)
          {
            found.push(r);
          }
        },
        // A lock with no snapshot is left over: remove it if it is free.
        Some("lock") if !path.with_extension("recover").exists() => {
          if let Some(held) = take_lock(&path) {
            drop(held);
            let _ = std::fs::remove_file(&path);
          }
        },
        _ => {},
      }
    }
    found.sort_by_key(|a| std::cmp::Reverse(a.written));
    found
  }
}

/// Locks `path`, made if missing, when no one else holds it.
fn take_lock(path: &Path) -> Option<File> {
  let file = OpenOptions::new()
    .create(true)
    .truncate(false)
    .write(true)
    .open(path)
    .ok()?;
  match file.try_lock() {
    Ok(()) => Some(file),
    Err(TryLockError::WouldBlock | TryLockError::Error(_)) => None,
  }
}

/// Unsaved work left by a program that stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovered {
  /// The snapshot file.
  pub file: PathBuf,
  /// The kind of document, as in "Board".
  pub kind: String,
  /// The file the document was last saved as or opened from.
  pub original: Option<PathBuf>,
  /// When the snapshot was written.
  pub written: SystemTime,
  /// The document, as saving would write it.
  pub bytes: Vec<u8>,
}

impl Recovered {
  /// Removes the snapshot, after it was recovered or turned down.
  pub fn discard(&self) {
    let _ = std::fs::remove_file(&self.file);
    let _ = std::fs::remove_file(self.file.with_extension("lock"));
  }

  /// The name to show: the original file's name, or "Untitled".
  pub fn name(&self) -> String {
    self
      .original
      .as_ref()
      .and_then(|p| p.file_name())
      .map_or_else(
        || "Untitled".to_owned(),
        |n| n.to_string_lossy().into_owned(),
      )
  }
}

fn parse(
  file: &Path,
  bytes: &[u8],
) -> Option<Recovered> {
  let split = bytes.windows(2).position(|w| w == b"\n\n")?;
  let head = std::str::from_utf8(&bytes[..split]).ok()?;
  let mut lines = head.lines();
  if lines.next()? != HEADER {
    return None;
  }
  let mut kind = String::new();
  let mut original = None;
  let mut written = UNIX_EPOCH;
  for line in lines {
    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
    match key {
      "kind" => kind = value.to_owned(),
      "path" if !value.is_empty() => original = Some(PathBuf::from(value)),
      "written" => {
        written = UNIX_EPOCH + Duration::from_millis(value.parse().ok()?);
      },
      _ => {},
    }
  }
  Some(Recovered {
    file: file.to_owned(),
    kind,
    original,
    written,
    bytes: bytes[split + 2..].to_vec(),
  })
}

/// One document's snapshots, while it has unsaved changes.
pub(crate) struct Snapshots {
  stem: PathBuf,
  lock: Option<File>,
  /// The first change not yet in a snapshot.
  pub(crate) pending_since: Option<Instant>,
  /// The timer that writes a snapshot once edits stop.
  pub(crate) timer: Option<dovetail::Task>,
}

impl Snapshots {
  /// Takes a new name in the folder, and locks it.
  pub(crate) fn start(recovery: &Recovery) -> io::Result<Snapshots> {
    std::fs::create_dir_all(&recovery.dir)?;
    thread_local! {
      static COUNT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    let n = COUNT.get();
    COUNT.set(n + 1);
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_millis();
    let stem = recovery
      .dir
      .join(format!("{}-{now}-{n}", std::process::id()));
    let lock = take_lock(&stem.with_extension("lock")).ok_or_else(|| {
      io::Error::new(io::ErrorKind::WouldBlock, "the recovery lock is held")
    })?;
    Ok(Snapshots {
      stem,
      lock: Some(lock),
      pending_since: None,
      timer: None,
    })
  }

  pub(crate) fn file(&self) -> PathBuf {
    self.stem.with_extension("recover")
  }

  /// Writes a snapshot: to a new file first, which then replaces the old
  /// one, so a crash while writing leaves the last snapshot whole.
  pub(crate) fn write(
    &mut self,
    kind: &str,
    original: Option<&Path>,
    bytes: &[u8],
  ) -> io::Result<()> {
    let written = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_millis();
    let mut out = format!(
      "{HEADER}\nkind {kind}\npath {}\nwritten {written}\n\n",
      original.map_or_else(String::new, |p| p.display().to_string())
    )
    .into_bytes();
    out.extend_from_slice(bytes);
    let fresh = self.stem.with_extension("recover-new");
    std::fs::write(&fresh, &out)?;
    std::fs::rename(&fresh, self.file())?;
    self.pending_since = None;
    Ok(())
  }

  /// Removes the snapshot, as the work was saved or thrown away.
  pub(crate) fn clear(&mut self) {
    if let Some(t) = self.timer.take() {
      t.cancel();
    }
    self.pending_since = None;
    let _ = std::fs::remove_file(self.file());
  }
}

impl Drop for Snapshots {
  fn drop(&mut self) {
    if let Some(t) = self.timer.take() {
      t.cancel();
    }
    // The lock is let go; it is removed when there is no snapshot to keep.
    drop(self.lock.take());
    if !self.file().exists() {
      let _ = std::fs::remove_file(self.stem.with_extension("lock"));
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
      "dovetail-recovery-test-{name}-{}",
      std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
  }

  #[test]
  fn held_snapshots_are_not_offered_and_left_ones_are() {
    let dir = folder("held");
    let recovery = Recovery::new(&dir);
    let mut held = Snapshots::start(&recovery).unwrap();
    held
      .write(
        "Board",
        Some(Path::new("/a/main.board")),
        b"part R1\n\nmore",
      )
      .unwrap();
    assert!(held.file().exists());
    assert!(recovery.found().is_empty(), "held by this program");

    // A program that stops lets go of its lock but leaves the snapshot.
    let file = held.file();
    drop(held.lock.take());
    let found = recovery.found();
    assert_eq!(found.len(), 1, "{found:?}");
    let r = &found[0];
    assert_eq!(r.file, file);
    assert_eq!(r.kind, "Board");
    assert_eq!(r.original.as_deref(), Some(Path::new("/a/main.board")));
    assert_eq!(r.bytes, b"part R1\n\nmore", "blank lines in the body stay");
    assert_eq!(r.name(), "main.board");
    assert!(r.written > UNIX_EPOCH);
    r.discard();
    assert!(recovery.found().is_empty());
    drop(held);
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn cleared_snapshots_leave_nothing() {
    let dir = folder("cleared");
    let recovery = Recovery::new(&dir);
    let mut s = Snapshots::start(&recovery).unwrap();
    s.write("Board", None, b"x").unwrap();
    s.clear();
    drop(s);
    assert_eq!(
      std::fs::read_dir(&dir).unwrap().count(),
      0,
      "no snapshot and no lock are left"
    );
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn files_that_are_not_snapshots_are_ignored() {
    let dir = folder("bad");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.recover"), b"something else\n\nx").unwrap();
    std::fs::write(dir.join("b.recover"), b"no blank line").unwrap();
    std::fs::write(dir.join("notes.txt"), b"hello").unwrap();
    assert!(Recovery::new(&dir).found().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
  }
}
