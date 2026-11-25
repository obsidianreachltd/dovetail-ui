//! Where applications keep settings, such as a saved docking layout (M10).

use std::path::PathBuf;

/// The folder for `app`'s settings in the user's settings folder, which
/// may not exist yet:
///
/// - Windows: `%APPDATA%\app`;
/// - macOS: `~/Library/Application Support/app`;
/// - Linux and others: `$XDG_CONFIG_HOME/app`, or `~/.config/app`.
///
/// `None` when the system does not say where the user's folders are.
pub fn settings_dir(app: &str) -> Option<PathBuf> {
  let var = |name: &str| {
    std::env::var_os(name)
      .filter(|v| !v.is_empty())
      .map(PathBuf::from)
  };
  let base = if cfg!(windows) {
    var("APPDATA")
  } else if cfg!(target_os = "macos") {
    var("HOME").map(|h| h.join("Library").join("Application Support"))
  } else {
    var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config")))
  };
  Some(base?.join(app))
}
