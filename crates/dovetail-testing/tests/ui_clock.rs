//! D0027: code on the UI thread reads the time from `dovetail_core::now()`,
//! so tests can move it. This scans the packages that run on the UI
//! thread for `Instant::now()`.
//!
//! A line that must read the real time says why in a `// real time:`
//! comment on it or the line before, as the clock itself and the input
//! log do. Test code, after `#[cfg(test)]`, is not scanned.

use std::path::{Path, PathBuf};

/// The source folders of code that runs on the UI thread.
const FOLDERS: [&str; 11] = [
  "dovetail-core/src",
  "dovetail-layout/src",
  "dovetail-session/src",
  "dovetail-desktop/src/app",
  "dovetail-controls/src",
  "dovetail-commands/src",
  "dovetail-data/src",
  "dovetail-docking/src",
  "dovetail-documents/src",
  "dovetail-theme/src",
  "dovetail-image/src",
];

/// Single files, where their folder also holds other threads' code.
const FILES: [&str; 1] = ["dovetail-desktop/src/app.rs"];

fn rust_files(
  dir: &Path,
  out: &mut Vec<PathBuf>,
) {
  for entry in std::fs::read_dir(dir)
    .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
    .flatten()
  {
    let path = entry.path();
    if path.is_dir() {
      rust_files(&path, out);
    } else if path.extension().is_some_and(|e| e == "rs") {
      out.push(path);
    }
  }
}

#[test]
fn ui_thread_code_reads_the_time_from_the_clock() {
  let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
  let mut paths = Vec::new();
  for folder in FOLDERS {
    rust_files(&crates.join(folder), &mut paths);
  }
  paths.extend(FILES.iter().map(|f| crates.join(f)));
  let mut found = Vec::new();
  for path in &paths {
    let text = std::fs::read_to_string(path).unwrap();
    let mut before = "";
    for (n, line) in text.lines().enumerate() {
      if line.trim_start().starts_with("#[cfg(test)]") {
        break;
      }
      let code = line.split("//").next().unwrap_or("");
      let excused =
        line.contains("// real time:") || before.contains("// real time:");
      if code.contains("Instant::now()") && !excused {
        found.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
      }
      before = line;
    }
  }
  assert!(
    paths.len() >= 40,
    "the sources were found ({})",
    paths.len()
  );
  assert!(
    found.is_empty(),
    "read the time with dovetail_core::now(), or say why not with a \
     `// real time:` comment:\n{}",
    found.join("\n")
  );
}
