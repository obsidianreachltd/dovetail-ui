//! M05 "Done when" items 1 and 4: a new project builds with no changes, in
//! debug and release. M20 "Done when" item 6: its test passes. It compiles
//! a whole project, including wgpu, so it is ignored by default:
//! `cargo test -p dovetail-cli --test new_project -- --ignored`.
//!
//! The project's target folder is shared between runs, in this workspace's
//! `target/new-project-test`, so only the first run is slow.

use std::path::{Path, PathBuf};
use std::process::Command;

fn dovetail(
  folder: &Path,
  args: &[&str],
) -> std::process::Output {
  Command::new(env!("CARGO_BIN_EXE_dovetail"))
    .current_dir(folder)
    .args(args)
    .env("CARGO_TARGET_DIR", shared_target())
    .output()
    .expect("run dovetail")
}

fn shared_target() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/new-project-test")
}

#[test]
#[ignore = "builds a whole project; slow the first time"]
fn a_new_project_builds_in_debug_and_release_with_no_changes() {
  let parent = std::env::temp_dir()
    .join(format!("dovetail-new-test-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&parent);
  std::fs::create_dir_all(&parent).unwrap();

  let created = dovetail(&parent, &["new", "sample-app"]);
  assert!(
    created.status.success(),
    "{}",
    String::from_utf8_lossy(&created.stderr)
  );
  let project = parent.join("sample-app");

  for (args, profile) in [
    (&["build"][..], "debug"),
    (&["build", "--release"][..], "release"),
  ] {
    let built = dovetail(&project, args);
    let stdout = String::from_utf8_lossy(&built.stdout);
    assert!(
      built.status.success(),
      "{args:?} failed:\n{}",
      String::from_utf8_lossy(&built.stderr)
    );
    let program = stdout
      .lines()
      .find_map(|l| l.strip_prefix("Built "))
      .map(PathBuf::from)
      .unwrap_or_else(|| panic!("no `Built` line in {stdout:?}"));
    assert!(program.is_file(), "{} exists", program.display());
    assert!(
      program.to_string_lossy().contains(profile),
      "{} is a {profile} build",
      program.display()
    );
  }

  // The project's own test passes (M20).
  let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
  let tested = Command::new(cargo)
    .current_dir(&project)
    .arg("test")
    .env("CARGO_TARGET_DIR", shared_target())
    .output()
    .expect("run cargo test");
  let out = String::from_utf8_lossy(&tested.stdout);
  assert!(
    tested.status.success() && out.contains("test result: ok. 1 passed"),
    "the new project's test:
{out}
{}",
    String::from_utf8_lossy(&tested.stderr)
  );

  let again = dovetail(&parent, &["new", "sample-app"]);
  assert!(!again.status.success());
  assert!(String::from_utf8_lossy(&again.stderr).contains("already exists"));
  std::fs::remove_dir_all(&parent).unwrap();
}
