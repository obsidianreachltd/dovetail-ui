//! The system's settings for behaviour (M18, decision D0028), made from
//! what each system's own functions give. `platform/` reads the values;
//! this turns them into a `SystemSettings`. It is built on every system,
//! so each system's rules are tested everywhere.

// Each system uses only its own part, outside tests.
#![allow(dead_code)]

use std::time::Duration;

use dovetail_core::SystemSettings;

/// What Windows gives, from `SystemParametersInfo`, the registry,
/// `GetCaretBlinkTime`, `GetDoubleClickTime` and `GetSystemMetrics`.
pub(crate) struct WindowsValues {
  /// `SPI_GETCLIENTAREAANIMATION`, or `None` when it could not be read.
  pub animation: Option<bool>,
  /// `TextScaleFactor` under `HKCU\Software\Microsoft\Accessibility`, a
  /// percentage, or `None` when it is not there.
  pub text_scale_percent: Option<u32>,
  /// `GetCaretBlinkTime`: milliseconds, `u32::MAX` for no blinking, 0 on
  /// failure.
  pub caret_blink_ms: u32,
  /// `GetDoubleClickTime`, in milliseconds.
  pub double_click_ms: u32,
  /// `SM_CXDOUBLECLK` and `SM_CYDOUBLECLK`: the width and height of the
  /// rectangle round the first click that the second must fall in.
  pub double_click_rect: (i32, i32),
}

/// Windows' settings.
pub(crate) fn from_windows(v: &WindowsValues) -> SystemSettings {
  let default = SystemSettings::default();
  SystemSettings {
    reduce_motion: v.animation == Some(false),
    text_scale: v
      .text_scale_percent
      .map_or(1.0, |p| p as f32 / 100.0),
    caret_blink: match v.caret_blink_ms {
      u32::MAX => None,
      0 => default.caret_blink,
      ms => Some(Duration::from_millis(u64::from(ms))),
    },
    double_click_time: match v.double_click_ms {
      0 => default.double_click_time,
      ms => Duration::from_millis(u64::from(ms)),
    },
    // The rectangle is centred on the first click, so the pointer may
    // move half of it each way.
    double_click_distance: match v.double_click_rect {
      (w, h) if w > 0 && h > 0 => w.max(h) as f32 / 2.0,
      _ => default.double_click_distance,
    },
    overlay_scrollbars: false,
  }
}

/// What macOS gives, from AppKit.
pub(crate) struct MacValues {
  /// `NSWorkspace`'s `accessibilityDisplayShouldReduceMotion`.
  pub reduce_motion: bool,
  /// `NSEvent`'s `doubleClickInterval`, in seconds.
  pub double_click_seconds: f64,
  /// The `NSTextInsertionPointBlinkPeriodOn` default, in milliseconds,
  /// where the user has set it.
  pub blink_on_ms: Option<i64>,
  /// `NSScroller`'s `preferredScrollerStyle` is the overlay style.
  pub overlay_scrollbars: bool,
}

/// macOS's settings. macOS has no text scale for applications, and no
/// double-click distance, so they keep their defaults.
pub(crate) fn from_macos(v: &MacValues) -> SystemSettings {
  let default = SystemSettings::default();
  SystemSettings {
    reduce_motion: v.reduce_motion,
    caret_blink: match v.blink_on_ms {
      Some(ms) if ms > 0 => Some(Duration::from_millis(ms as u64)),
      _ => default.caret_blink,
    },
    double_click_time: if v.double_click_seconds.is_finite()
      && v.double_click_seconds > 0.0
    {
      Duration::from_secs_f64(v.double_click_seconds.min(5.0))
    } else {
      default.double_click_time
    },
    overlay_scrollbars: v.overlay_scrollbars,
    ..default
  }
}

/// A value as `gsettings get` prints it, without quotes or a type name
/// such as `uint32`.
fn value(printed: &str) -> &str {
  let v = printed.trim();
  let v = v.rsplit(' ').next().unwrap_or(v);
  v.trim_matches('\'')
}

/// GNOME's settings, from what `gsettings get` printed for each (`None`
/// where a setting could not be read):
///
/// - `org.gnome.desktop.interface`: `enable-animations`,
///   `text-scaling-factor`, `cursor-blink` and `cursor-blink-time`, which
///   is a whole blink, on and off;
/// - `org.gnome.desktop.peripherals.mouse`: `double-click`, in
///   milliseconds.
///
/// GNOME has no double-click distance, and does not overlay scroll bars
/// for other toolkits, so they keep their defaults.
pub(crate) fn parse_gnome(
  enable_animations: Option<&str>,
  text_scaling_factor: Option<&str>,
  cursor_blink: Option<&str>,
  cursor_blink_time: Option<&str>,
  double_click: Option<&str>,
) -> SystemSettings {
  let default = SystemSettings::default();
  let number = |printed: Option<&str>| {
    printed
      .and_then(|p| value(p).parse::<f64>().ok())
      .filter(|n| n.is_finite() && *n > 0.0)
  };
  let blink = match cursor_blink.map(value) {
    Some("false") => None,
    _ => number(cursor_blink_time)
      .map(|ms| Duration::from_millis((ms / 2.0) as u64))
      .or(default.caret_blink),
  };
  SystemSettings {
    reduce_motion: enable_animations.map(value) == Some("false"),
    text_scale: number(text_scaling_factor).map_or(1.0, |n| n as f32),
    caret_blink: blink,
    double_click_time: number(double_click)
      .map_or(default.double_click_time, |ms| {
        Duration::from_millis(ms as u64)
      }),
    ..default
  }
}

/// GNOME's settings, read with `gsettings`.
pub(crate) fn from_gsettings() -> SystemSettings {
  let get = crate::appearance::gsettings;
  let interface = "org.gnome.desktop.interface";
  parse_gnome(
    get(interface, "enable-animations").as_deref(),
    get(interface, "text-scaling-factor").as_deref(),
    get(interface, "cursor-blink").as_deref(),
    get(interface, "cursor-blink-time").as_deref(),
    get("org.gnome.desktop.peripherals.mouse", "double-click").as_deref(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
  }

  #[test]
  fn windows_settings_are_read() {
    let usual = WindowsValues {
      animation: Some(true),
      text_scale_percent: None,
      caret_blink_ms: 530,
      double_click_ms: 500,
      double_click_rect: (4, 4),
    };
    let s = from_windows(&usual);
    assert!(!s.reduce_motion);
    assert_eq!(s.text_scale, 1.0);
    assert_eq!(s.caret_blink, Some(ms(530)));
    assert_eq!(s.double_click_time, ms(500));
    assert_eq!(s.double_click_distance, 2.0);
    assert!(!s.overlay_scrollbars);

    let changed = from_windows(&WindowsValues {
      animation: Some(false),
      text_scale_percent: Some(150),
      caret_blink_ms: u32::MAX,
      double_click_ms: 900,
      double_click_rect: (8, 6),
    });
    assert!(changed.reduce_motion);
    assert_eq!(changed.text_scale, 1.5);
    assert_eq!(changed.caret_blink, None);
    assert_eq!(changed.double_click_time, ms(900));
    assert_eq!(changed.double_click_distance, 4.0);

    // What could not be read keeps the defaults.
    let failed = from_windows(&WindowsValues {
      animation: None,
      text_scale_percent: None,
      caret_blink_ms: 0,
      double_click_ms: 0,
      double_click_rect: (0, 0),
    });
    assert_eq!(failed, SystemSettings::default());
  }

  #[test]
  fn mac_settings_are_read() {
    let s = from_macos(&MacValues {
      reduce_motion: true,
      double_click_seconds: 0.25,
      blink_on_ms: Some(800),
      overlay_scrollbars: true,
    });
    assert!(s.reduce_motion && s.overlay_scrollbars);
    assert_eq!(s.double_click_time, ms(250));
    assert_eq!(s.caret_blink, Some(ms(800)));
    assert_eq!(s.text_scale, 1.0);

    let unset = from_macos(&MacValues {
      reduce_motion: false,
      double_click_seconds: f64::NAN,
      blink_on_ms: None,
      overlay_scrollbars: false,
    });
    assert_eq!(unset, SystemSettings::default());
  }

  #[test]
  fn gnome_settings_are_read() {
    let s = parse_gnome(
      Some("false\n"),
      Some("1.25\n"),
      Some("true\n"),
      Some("1200\n"),
      Some("400\n"),
    );
    assert!(s.reduce_motion);
    assert_eq!(s.text_scale, 1.25);
    assert_eq!(s.caret_blink, Some(ms(600)), "half of a whole blink");
    assert_eq!(s.double_click_time, ms(400));

    // gsettings may print a type name before a number.
    let typed = parse_gnome(None, None, None, None, Some("uint32 300\n"));
    assert_eq!(typed.double_click_time, ms(300));

    let off =
      parse_gnome(Some("true"), None, Some("false"), Some("1200"), None);
    assert!(!off.reduce_motion);
    assert_eq!(off.caret_blink, None);

    // Without gsettings, or with nonsense, the defaults stay.
    assert_eq!(
      parse_gnome(None, None, None, None, None),
      SystemSettings::default()
    );
    assert_eq!(
      parse_gnome(Some("x"), Some("-2"), Some("x"), Some("x"), Some("0")),
      SystemSettings::default()
    );
  }
}
