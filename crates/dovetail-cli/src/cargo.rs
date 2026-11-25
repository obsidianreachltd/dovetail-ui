//! Running `cargo build` and finding the program it made.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Finds the project folder: the current folder or a parent with a
/// `Cargo.toml`.
pub(crate) fn project_root() -> Result<PathBuf, String> {
  let start = std::env::current_dir()
    .map_err(|e| format!("cannot read the current folder: {e}"))?;
  start
    .ancestors()
    .find(|dir| dir.join("Cargo.toml").is_file())
    .map(Path::to_path_buf)
    .ok_or_else(|| "this is not inside a project; run `dovetail new <name>` first, or `cd` into one".to_owned())
}

/// The result of one build.
pub(crate) enum Built {
  /// The build worked and made this program.
  Program(PathBuf),
  /// The build failed. Cargo has already shown why.
  Failed,
}

/// Builds the project in `root`. Cargo's messages, including errors, go to
/// the terminal as usual; the program's path is read from its JSON
/// messages.
pub(crate) fn build(
  root: &Path,
  release: bool,
) -> Result<Built, String> {
  let mut command =
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
  command
    .current_dir(root)
    .args([
      "build",
      "--message-format=json-diagnostic-rendered-ansi",
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit());
  if release {
    command.arg("--release");
  }
  let mut child = command
    .spawn()
    .map_err(|e| format!("cannot start cargo: {e}"))?;
  let mut program = None;
  for line in BufReader::new(child.stdout.take().expect("piped")).lines() {
    let line = line.map_err(|e| format!("cannot read cargo's output: {e}"))?;
    if let Some(path) = executable(&line) {
      program = Some(path);
    } else if let Some(rendered) = rendered_message(&line) {
      // Compiler messages arrive as JSON; show them as cargo would.
      eprint!("{rendered}");
    }
  }
  let status = child
    .wait()
    .map_err(|e| format!("cargo did not finish: {e}"))?;
  match (status.success(), program) {
    (true, Some(program)) => Ok(Built::Program(program)),
    (true, None) => Err(
      "the build worked, but made no program; is this a library?".to_owned(),
    ),
    (false, _) => Ok(Built::Failed),
  }
}

/// `dovetail build [--release]`.
pub fn build_command(release: bool) -> Result<(), String> {
  let root = project_root()?;
  match build(&root, release)? {
    Built::Program(program) => {
      println!("Built {}", program.display());
      Ok(())
    },
    Built::Failed => Err("the build failed; the errors are above".to_owned()),
  }
}

/// The program path from a `compiler-artifact` message, if it has one.
pub(crate) fn executable(line: &str) -> Option<PathBuf> {
  if !line.contains(r#""reason":"compiler-artifact""#) {
    return None;
  }
  let value = json_string_after(line, r#""executable":"#)?;
  Some(PathBuf::from(value))
}

/// The `rendered` text of a `compiler-message`, if it has one.
fn rendered_message(line: &str) -> Option<String> {
  if !line.contains(r#""reason":"compiler-message""#) {
    return None;
  }
  json_string_after(line, r#""rendered":"#)
}

/// The first string value of `key` in `line`, unescaped. Values that are
/// not strings, such as `null` in a message's children, are skipped.
fn json_string_after(
  line: &str,
  key: &str,
) -> Option<String> {
  let with_quote = format!("{key}\"");
  let start = line.find(&with_quote)? + with_quote.len();
  let rest = &line[start..];
  let mut out = String::new();
  let mut chars = rest.chars();
  while let Some(c) = chars.next() {
    match c {
      '"' => return Some(out),
      '\\' => match chars.next()? {
        'n' => out.push('\n'),
        't' => out.push('\t'),
        'r' => out.push('\r'),
        'u' => {
          let hex: String = chars.by_ref().take(4).collect();
          out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
        },
        other => out.push(other),
      },
      c => out.push(c),
    }
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_program_path_is_read_from_cargo_json() {
    let line = r#"{"reason":"compiler-artifact","package_id":"x","target":{"kind":["bin"]},"executable":"C:\\work\\my-app\\target\\debug\\my-app.exe","fresh":true}"#;
    assert_eq!(
      executable(line),
      Some(PathBuf::from(r"C:\work\my-app\target\debug\my-app.exe"))
    );
  }

  #[test]
  fn libraries_and_other_messages_have_no_program() {
    let library = r#"{"reason":"compiler-artifact","executable":null}"#;
    assert_eq!(executable(library), None);
    assert_eq!(
      executable(r#"{"reason":"build-finished","success":true}"#),
      None
    );
  }

  #[test]
  fn compiler_messages_are_unescaped() {
    let line = r#"{"reason":"compiler-message","message":{"children":[{"rendered":null}],"rendered":"error: oops\n  --> src\\main.rs:1:1\n\u001b[0m"}}"#;
    assert_eq!(
      rendered_message(line).unwrap(),
      "error: oops\n  --> src\\main.rs:1:1\n\u{1b}[0m"
    );
  }
}
