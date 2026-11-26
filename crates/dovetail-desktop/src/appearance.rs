//! The system's appearance on Linux and other Unix systems (M14): GNOME's
//! settings, read with the `gsettings` program. Desktops without it are
//! taken to be light, with no accent colour and no high contrast. The
//! parsing is built on every system, so it is tested everywhere.

// Used only on Linux and other Unix systems, and in tests.
#![cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]

use dovetail_core::{Color, SystemAppearance};

/// GNOME's accent colours (GNOME 47), by the names its setting uses.
fn accent(name: &str) -> Option<Color> {
  Some(Color::hex(match name {
    "blue" => 0x3584e4,
    "teal" => 0x2190a4,
    "green" => 0x3a944a,
    "yellow" => 0xc88800,
    "orange" => 0xed5b00,
    "red" => 0xe62d42,
    "pink" => 0xd56199,
    "purple" => 0x9141ac,
    "slate" => 0x6f8396,
    _ => return None,
  }))
}

/// A value as `gsettings get` prints it, without quotes.
fn value(printed: &str) -> &str {
  printed.trim().trim_matches('\'')
}

/// The appearance from the three settings, as `gsettings get` printed
/// them (`None` where a setting could not be read).
pub(crate) fn parse(
  color_scheme: Option<&str>,
  accent_color: Option<&str>,
  high_contrast: Option<&str>,
) -> SystemAppearance {
  SystemAppearance {
    dark: color_scheme.is_some_and(|v| value(v) == "prefer-dark"),
    accent: accent_color.and_then(|v| accent(value(v))),
    high_contrast: high_contrast.is_some_and(|v| value(v) == "true"),
  }
}

pub(crate) fn gsettings(
  schema: &str,
  key: &str,
) -> Option<String> {
  let out = std::process::Command::new("gsettings")
    .args(["get", schema, key])
    .stderr(std::process::Stdio::null())
    .output()
    .ok()?;
  out
    .status
    .success()
    .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The appearance from GNOME's settings.
pub(crate) fn from_gsettings() -> SystemAppearance {
  let interface = "org.gnome.desktop.interface";
  parse(
    gsettings(interface, "color-scheme").as_deref(),
    gsettings(interface, "accent-color").as_deref(),
    gsettings("org.gnome.desktop.a11y.interface", "high-contrast").as_deref(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn gnome_settings_are_read() {
    let a = parse(Some("'prefer-dark'\n"), Some("'teal'\n"), Some("true\n"));
    assert!(a.dark && a.high_contrast);
    assert_eq!(a.accent, Some(Color::hex(0x2190a4)));
    let b = parse(Some("'default'"), Some("'nonsense'"), Some("false"));
    assert_eq!(b, SystemAppearance::default());
    assert_eq!(parse(None, None, None), SystemAppearance::default());
  }
}
