//! M14 "Done when" item 2: no fixed colours are left in the control
//! packages. Every colour they draw comes from the theme. Transparent is
//! not a colour choice, so it is allowed.

use std::path::{Path, PathBuf};

const PACKAGES: [&str; 4] = [
  "dovetail-controls",
  "dovetail-commands",
  "dovetail-docking",
  "dovetail-documents",
];

/// Ways of writing a fixed colour.
const FIXED: [&str; 6] = [
  "Color::hex(",
  "Color::rgb(",
  "Color::WHITE",
  "Color::BLACK",
  "Color { r:",
  "const ACCENT",
];

fn rust_files(
  dir: &Path,
  out: &mut Vec<PathBuf>,
) {
  for entry in std::fs::read_dir(dir)
    .expect("a source folder")
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
fn control_packages_take_every_colour_from_the_theme() {
  let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
  let mut found = Vec::new();
  let mut files = 0;
  for package in PACKAGES {
    let mut paths = Vec::new();
    rust_files(&crates.join(package).join("src"), &mut paths);
    for path in paths {
      files += 1;
      let text = std::fs::read_to_string(&path).unwrap();
      for (n, line) in text.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for fixed in FIXED {
          if code.contains(fixed) {
            found.push(format!(
              "{}:{}: {}",
              path.display(),
              n + 1,
              line.trim()
            ));
          }
        }
      }
    }
  }
  assert!(files >= 10, "the sources were found ({files} files)");
  assert!(found.is_empty(), "fixed colours:\n{}", found.join("\n"));
}
