//! The `dovetail` command: create, run and build Dovetail UI applications.
//!
//! - `dovetail new <name>` creates a small working project.
//! - `dovetail dev` builds and runs the project, and rebuilds and restarts it
//!   whenever a source file is saved.
//! - `dovetail build [--release]` builds the project and prints where the
//!   program is.

mod cargo;
mod dev;
mod new;

use std::process::ExitCode;

const USAGE: &str = "\
Usage: dovetail <command>

Commands:
  new <name>          Create a new application in the folder <name>
  dev                 Run the application, and rebuild and restart it on save
  build [--release]   Build the application
  --version           Show the version
  --help              Show this help

Run `dev` and `build` inside a project folder (one with a Cargo.toml).";

/// What the user asked for.
#[derive(Debug, PartialEq, Eq)]
enum Command {
  New(String),
  Dev,
  Build { release: bool },
  Version,
  Help,
}

/// Turns the arguments (without the program name) into a command.
fn parse(args: &[String]) -> Result<Command, String> {
  let words: Vec<&str> = args.iter().map(String::as_str).collect();
  match words.as_slice() {
    [] | ["--help" | "-h" | "help"] => Ok(Command::Help),
    ["--version" | "-V"] => Ok(Command::Version),
    ["new", name] if !name.starts_with('-') => {
      Ok(Command::New((*name).to_owned()))
    },
    ["new"] => {
      Err("`new` needs a name, for example `dovetail new my-app`".to_owned())
    },
    ["dev"] => Ok(Command::Dev),
    ["build"] => Ok(Command::Build { release: false }),
    ["build", "--release" | "-r"] => Ok(Command::Build { release: true }),
    [command, rest @ ..] if ["new", "dev", "build"].contains(command) => {
      Err(format!("`{command}` does not take `{}`", rest.join(" ")))
    },
    [other, ..] => Err(format!("`{other}` is not a dovetail command")),
  }
}

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let command = match parse(&args) {
    Ok(command) => command,
    Err(message) => {
      eprintln!("dovetail: {message}\n\n{USAGE}");
      return ExitCode::FAILURE;
    },
  };
  let result = match command {
    Command::Help => {
      println!("{USAGE}");
      Ok(())
    },
    Command::Version => {
      println!("dovetail {}", env!("CARGO_PKG_VERSION"));
      Ok(())
    },
    Command::New(name) => new::run(&name),
    Command::Build { release } => cargo::build_command(release),
    Command::Dev => dev::run(),
  };
  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(message) => {
      eprintln!("dovetail: {message}");
      ExitCode::FAILURE
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
  }

  #[test]
  fn commands_are_recognised() {
    assert_eq!(parse(&args(&[])), Ok(Command::Help));
    assert_eq!(parse(&args(&["--version"])), Ok(Command::Version));
    assert_eq!(
      parse(&args(&["new", "my-app"])),
      Ok(Command::New("my-app".into()))
    );
    assert_eq!(parse(&args(&["dev"])), Ok(Command::Dev));
    assert_eq!(
      parse(&args(&["build"])),
      Ok(Command::Build { release: false })
    );
    assert_eq!(
      parse(&args(&["build", "--release"])),
      Ok(Command::Build { release: true })
    );
  }

  #[test]
  fn mistakes_get_clear_messages() {
    assert!(
      parse(&args(&["new"]))
        .unwrap_err()
        .contains("needs a name")
    );
    assert!(
      parse(&args(&["build", "--fast"]))
        .unwrap_err()
        .contains("does not take `--fast`")
    );
    assert!(
      parse(&args(&["run"]))
        .unwrap_err()
        .contains("not a dovetail command")
    );
    assert!(parse(&args(&["new", "--release"])).is_err());
  }
}
