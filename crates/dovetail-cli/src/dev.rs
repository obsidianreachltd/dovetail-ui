//! `dovetail dev`: build, run, and rebuild and restart on every save.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime};

use crate::cargo::{self, Built};

/// How often files are checked for changes.
const POLL: Duration = Duration::from_millis(200);
/// How long to wait for the program's first frame before giving up on
/// measuring it.
const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// The modification time of every watched file.
type Snapshot = BTreeMap<PathBuf, SystemTime>;

/// What `dovetail dev` watches: `src/`, `assets/` and `Cargo.toml`.
pub(crate) fn snapshot(root: &Path) -> Snapshot {
  let mut files = Snapshot::new();
  let mut add = |path: &Path| {
    if let Ok(modified) = path.metadata().and_then(|m| m.modified()) {
      files.insert(path.to_path_buf(), modified);
    }
  };
  add(&root.join("Cargo.toml"));
  let mut folders = vec![root.join("src"), root.join("assets")];
  while let Some(folder) = folders.pop() {
    let Ok(entries) = std::fs::read_dir(&folder) else {
      continue;
    };
    for entry in entries.flatten() {
      let path = entry.path();
      if path.is_dir() {
        folders.push(path);
      } else {
        add(&path);
      }
    }
  }
  files
}

/// The files added, removed or changed between two snapshots.
pub(crate) fn changed(
  before: &Snapshot,
  after: &Snapshot,
) -> Vec<PathBuf> {
  let mut changed: Vec<PathBuf> = after
    .iter()
    .filter(|(path, time)| before.get(*path) != Some(time))
    .map(|(path, _)| path.clone())
    .collect();
  changed.extend(
    before
      .keys()
      .filter(|path| !after.contains_key(*path))
      .cloned(),
  );
  changed
}

/// The running program, and the files `dovetail dev` shares with it.
struct Running {
  child: Child,
}

struct Session {
  root: PathBuf,
  dev_dir: PathBuf,
  state_file: PathBuf,
  ready_file: PathBuf,
  runs: u32,
  running: Option<Running>,
}

impl Session {
  /// Copies the program, so the next build can replace the original while
  /// this copy runs (Windows locks a running program's file).
  fn copy_program(
    &mut self,
    program: &Path,
  ) -> Result<PathBuf, String> {
    self.runs += 1;
    let name = program
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or("app");
    let extension = program
      .extension()
      .and_then(|e| e.to_str())
      .map(|e| format!(".{e}"))
      .unwrap_or_default();
    // Two names in turn: the one in use, and the one being replaced.
    let copy = self
      .dev_dir
      .join(format!("{name}-dev-{}{extension}", self.runs % 2));
    std::fs::copy(program, &copy).map_err(|e| {
      format!("cannot copy the program to `{}`: {e}", copy.display())
    })?;
    Ok(copy)
  }

  fn stop(&mut self) {
    if let Some(mut running) = self.running.take() {
      let _ = running.child.kill();
      let _ = running.child.wait();
    }
  }

  fn start(
    &mut self,
    program: &Path,
  ) -> Result<(), String> {
    let _ = std::fs::remove_file(&self.ready_file);
    let child = Command::new(program)
      .current_dir(&self.root)
      .env("DOVETAIL_DEV_STATE", &self.state_file)
      .env("DOVETAIL_DEV_READY", &self.ready_file)
      .spawn()
      .map_err(|e| format!("cannot start `{}`: {e}", program.display()))?;
    self.running = Some(Running { child });
    Ok(())
  }

  /// Waits for the program's first frame. `None` if it exited or took too
  /// long.
  fn wait_until_ready(&mut self) -> Option<Instant> {
    let deadline = Instant::now() + READY_TIMEOUT;
    while Instant::now() < deadline {
      if self.ready_file.exists() {
        return Some(Instant::now());
      }
      if let Some(running) = &mut self.running
        && let Ok(Some(status)) = running.child.try_wait()
      {
        println!(
          "dovetail: the application exited ({status}) before showing a window"
        );
        self.running = None;
        return None;
      }
      sleep(Duration::from_millis(10));
    }
    None
  }

  /// Builds; on success replaces the running program. Returns whether it
  /// built.
  fn rebuild(
    &mut self,
    saved_at: Instant,
  ) -> Result<bool, String> {
    let build_started = Instant::now();
    match cargo::build(&self.root, false)? {
      Built::Failed => {
        let still = if self.running.is_some() {
          " The last working version is still running."
        } else {
          ""
        };
        println!(
          "\ndovetail: the build failed; the errors are above.{still}\n\
           dovetail: fix them and save to try again.\n"
        );
        Ok(false)
      },
      Built::Program(program) => {
        let built = build_started.elapsed();
        let copy = self.copy_program(&program)?;
        self.stop();
        self.start(&copy)?;
        match self.wait_until_ready() {
          Some(ready) => println!(
            "dovetail: restarted {:.1} s after the save (build {:.1} s, start {:.1} s)",
            (ready - saved_at).as_secs_f64(),
            built.as_secs_f64(),
            (ready - build_started - built).as_secs_f64(),
          ),
          None if self.running.is_some() => println!(
            "dovetail: started; no window was shown within {READY_TIMEOUT:?}"
          ),
          None => {},
        }
        Ok(true)
      },
    }
  }
}

impl Drop for Session {
  fn drop(&mut self) {
    self.stop();
  }
}

pub fn run() -> Result<(), String> {
  let root = cargo::project_root()?;
  let dev_dir = root.join("target").join("dovetail-dev");
  std::fs::create_dir_all(&dev_dir)
    .map_err(|e| format!("cannot create `{}`: {e}", dev_dir.display()))?;
  let mut session = Session {
    state_file: dev_dir.join("window.txt"),
    ready_file: dev_dir.join("ready.txt"),
    root,
    dev_dir,
    runs: 0,
    running: None,
  };
  println!("dovetail: building {}", session.root.display());
  let mut files = snapshot(&session.root);
  session.rebuild(Instant::now())?;
  println!(
    "dovetail: watching src/, assets/ and Cargo.toml; press Ctrl+C to stop"
  );

  loop {
    sleep(POLL);
    let now = snapshot(&session.root);
    let edits = changed(&files, &now);
    if edits.is_empty() {
      // Notice when the application is closed, but keep watching.
      if let Some(running) = &mut session.running
        && let Ok(Some(_)) = running.child.try_wait()
      {
        session.running = None;
        println!(
          "dovetail: the application was closed; save a file to start it again"
        );
      }
      continue;
    }
    // A save can arrive in pieces; wait for the files to settle.
    let saved_at = Instant::now();
    sleep(Duration::from_millis(50));
    files = snapshot(&session.root);
    let names: Vec<String> = edits
      .iter()
      .map(|p| {
        p.strip_prefix(&session.root)
          .unwrap_or(p)
          .display()
          .to_string()
      })
      .collect();
    println!("dovetail: {} changed; rebuilding", names.join(", "));
    session.rebuild(saved_at)?;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn changes_are_found_by_modification_time() {
    let t = |s| SystemTime::UNIX_EPOCH + Duration::from_secs(s);
    let before: Snapshot = [
      ("a.rs".into(), t(1)),
      ("b.rs".into(), t(1)),
      ("gone.rs".into(), t(1)),
    ]
    .into();
    let after: Snapshot = [
      ("a.rs".into(), t(1)),
      ("b.rs".into(), t(2)),
      ("new.rs".into(), t(1)),
    ]
    .into();
    let mut found = changed(&before, &after);
    found.sort();
    assert_eq!(
      found,
      [
        PathBuf::from("b.rs"),
        "gone.rs".into(),
        "new.rs".into()
      ]
    );
    assert!(changed(&after, &after).is_empty());
  }

  #[test]
  fn snapshot_watches_sources_assets_and_the_manifest_only() {
    let root = std::env::temp_dir()
      .join(format!("dovetail-dev-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for file in [
      "Cargo.toml",
      "src/main.rs",
      "src/ui/panel.rs",
      "assets/logo.svg",
      "target/debug/app",
      "README.md",
    ] {
      let path = root.join(file);
      std::fs::create_dir_all(path.parent().unwrap()).unwrap();
      std::fs::write(&path, "x").unwrap();
    }
    let mut watched: Vec<String> = snapshot(&root)
      .keys()
      .map(|p| {
        p.strip_prefix(&root)
          .unwrap()
          .to_string_lossy()
          .replace('\\', "/")
      })
      .collect();
    watched.sort();
    assert_eq!(
      watched,
      [
        "Cargo.toml",
        "assets/logo.svg",
        "src/main.rs",
        "src/ui/panel.rs"
      ]
    );
    std::fs::remove_dir_all(&root).unwrap();
  }
}
