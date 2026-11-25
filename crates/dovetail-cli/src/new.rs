//! `dovetail new <name>`: creates a small working project.

use std::path::{Path, PathBuf};

/// Where the `dovetail-ui` crate is, relative to this crate. New projects
/// depend on it by path until the crates are published (R01, the first release).
fn dovetail_ui_path() -> PathBuf {
  let here = Path::new(env!("CARGO_MANIFEST_DIR"));
  let path = here.join("..").join("dovetail-ui");
  let path = path.canonicalize().unwrap_or(path);
  // On Windows, `canonicalize` gives a `\\?\C:\...` path; Cargo wants the
  // ordinary form.
  match path
    .to_str()
    .and_then(|p| p.strip_prefix(r"\\?\"))
  {
    Some(plain) => PathBuf::from(plain),
    None => path,
  }
}

/// Checks that `name` can be a Cargo package name, and says why not.
pub(crate) fn check_name(name: &str) -> Result<(), String> {
  let mut chars = name.chars();
  let first = chars.next().ok_or("the name is empty")?;
  if !first.is_ascii_alphabetic() {
    return Err(format!("`{name}` must start with a letter"));
  }
  if let Some(bad) = name
    .chars()
    .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
  {
    return Err(format!(
      "`{name}` may only use letters, digits, `-` and `_`, not `{bad}`"
    ));
  }
  if [
    "dovetail",
    "dovetail-ui",
    "test",
    "std",
    "core",
    "alloc",
  ]
  .contains(&name)
  {
    return Err(format!("`{name}` is reserved; choose another name"));
  }
  Ok(())
}

/// A TOML string with backslashes and quotes escaped.
fn toml_string(text: &str) -> String {
  format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The files of a new project called `name`.
pub(crate) fn files(
  name: &str,
  dovetail_ui: &Path,
) -> Vec<(&'static str, String)> {
  let cargo_toml = format!(
    r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
# The Dovetail UI framework, used in code as `dovetail`. It comes from the
# Dovetail source by path until the crates are published.
dovetail = {{ package = "dovetail-ui", path = {path} }}

[dev-dependencies]
# Tests that run the window's components with no window, in `cargo test`.
dovetail-testing = {{ path = {testing} }}

# This project builds on its own, even inside another Cargo workspace.
[workspace]
"#,
    path = toml_string(&dovetail_ui.display().to_string()),
    testing = toml_string(
      &dovetail_ui
        .with_file_name("dovetail-testing")
        .display()
        .to_string()
    ),
  );
  let main_rs = format!(
    r#"use dovetail::prelude::*;

/// The application's window. Edit it while `dovetail dev` runs: each save
/// rebuilds and restarts it.
#[component]
fn App() -> View {{
    let count = use_state(|| 0);

    ui! {{
        Column {{
            Text {{
                value: count.map(|count| format!("Clicked {{count}} times")),
                size: Some(24.0),
            }}

            Button {{
                text: "Click me",
                on_click: move |_| count.update(|count| *count += 1),
            }}
        }}
    }}
}}

fn main() {{
    if let Err(error) = dovetail::run({title}, ui! {{ App {{}} }}) {{
        eprintln!("{{error}}");
        std::process::exit(1);
    }}
}}

/// Run these with `cargo test`. They run the window's components with no
/// window, find what is shown as a screen reader would, and click through
/// the window's own input.
#[cfg(test)]
mod tests {{
    use super::*;
    use dovetail_testing::{{TestUi, label, text}};

    #[test]
    fn clicking_the_button_counts() {{
        let mut ui = TestUi::new(ui! {{ App {{}} }});
        ui.find(text("Clicked 0 times"));
        ui.click(label("Click me"));
        ui.find(text("Clicked 1 times"));
    }}
}}
"#,
    // Names are letters, digits, `-` and `_` (`check_name`), so this is a
    // valid Rust string literal too.
    title = toml_string(name),
  );
  vec![
    ("Cargo.toml", cargo_toml),
    ("src/main.rs", main_rs),
    (".gitignore", "/target\n".to_owned()),
  ]
}

pub fn run(name: &str) -> Result<(), String> {
  check_name(name)?;
  let folder = Path::new(name);
  if folder.exists()
    && folder
      .read_dir()
      .map_err(|e| format!("cannot read `{name}`: {e}"))?
      .next()
      .is_some()
  {
    return Err(format!(
      "the folder `{name}` already exists and is not empty"
    ));
  }
  let dovetail_ui = dovetail_ui_path();
  if !dovetail_ui.join("Cargo.toml").exists() {
    return Err(format!(
      "cannot find the Dovetail source at `{}`; this `dovetail` was built from somewhere that has moved",
      dovetail_ui.display()
    ));
  }
  for (file, contents) in files(name, &dovetail_ui) {
    let path = folder.join(file);
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)
        .map_err(|e| format!("cannot create `{}`: {e}", parent.display()))?;
    }
    std::fs::write(&path, contents)
      .map_err(|e| format!("cannot write `{}`: {e}", path.display()))?;
  }
  println!("Created `{name}`. To run it:\n\n    cd {name}\n    dovetail dev\n");
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn good_names_pass() {
    for name in ["my-app", "pcb_studio", "App2"] {
      assert_eq!(check_name(name), Ok(()), "{name}");
    }
  }

  #[test]
  fn bad_names_say_why() {
    assert!(check_name("").unwrap_err().contains("empty"));
    assert!(
      check_name("2app")
        .unwrap_err()
        .contains("start with a letter")
    );
    assert!(
      check_name("my app")
        .unwrap_err()
        .contains("not ` `")
    );
    assert!(
      check_name("dovetail")
        .unwrap_err()
        .contains("reserved")
    );
  }

  #[test]
  fn the_project_depends_on_dovetail_by_path_and_stands_alone() {
    let files =
      files("my-app", Path::new(r"C:\src\dovetail\crates\dovetail-ui"));
    let cargo = &files
      .iter()
      .find(|(f, _)| *f == "Cargo.toml")
      .unwrap()
      .1;
    assert!(cargo.contains(r#"name = "my-app""#));
    assert!(
      cargo.contains(r#"path = "C:\\src\\dovetail\\crates\\dovetail-ui""#),
      "{cargo}"
    );
    assert!(cargo.contains("\n[workspace]\n"));
    let main = &files
      .iter()
      .find(|(f, _)| *f == "src/main.rs")
      .unwrap()
      .1;
    assert!(main.contains(r#"dovetail::run("my-app""#));
    assert!(main.contains("format!(\"Clicked {count} times\")"));
    // A test comes with it, and what it needs: the testing package, next
    // to dovetail-ui.
    assert!(cargo.contains("\n[dev-dependencies]\n"), "{cargo}");
    assert!(
      cargo.contains(
        r#"dovetail-testing = { path = "C:\\src\\dovetail\\crates\\dovetail-testing" }"#
      ),
      "{cargo}"
    );
    assert!(main.contains("fn clicking_the_button_counts()"));
  }
}
