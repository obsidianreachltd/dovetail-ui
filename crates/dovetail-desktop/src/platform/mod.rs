//! Platform code that winit, wgpu and AccessKit do not provide. The only
//! place in Dovetail where `unsafe` is allowed (D0013): each module below
//! wraps one operating system's interface and offers a safe one.

use std::rc::Rc;

use dovetail_core::Clipboard;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11;

/// The language the user reads the system in, as a tag such as `ja-JP` or
/// `zh_TW.UTF-8`: from Windows or macOS, and elsewhere from `LC_ALL`,
/// `LC_MESSAGES` or `LANG`.
pub(crate) fn language() -> Option<String> {
  #[cfg(windows)]
  {
    windows::language()
  }
  #[cfg(target_os = "macos")]
  {
    macos::language()
  }
  #[cfg(not(any(windows, target_os = "macos")))]
  {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
      .into_iter()
      .filter_map(|name| std::env::var(name).ok())
      .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
  }
}

/// The system menu bar, where the system has one Dovetail uses: macOS.
/// Elsewhere menus are drawn in the window.
pub(crate) fn native_menus() -> Option<Rc<dyn dovetail_core::NativeMenus>> {
  #[cfg(target_os = "macos")]
  {
    Some(Rc::new(macos::MacMenus))
  }
  #[cfg(not(target_os = "macos"))]
  {
    None
  }
}

/// The system's file dialogs (M11, D0017): its own on Windows and macOS,
/// and `zenity` or `kdialog` elsewhere, when one is installed. `owner`
/// holds the window in front, which owns the dialog on Windows.
pub(crate) fn file_dialogs(
  owner: Rc<std::cell::Cell<isize>>
) -> Option<Rc<dyn dovetail_core::FileDialogs>> {
  #[cfg(windows)]
  {
    Some(Rc::new(windows::WindowsFileDialogs { owner }))
  }
  #[cfg(target_os = "macos")]
  {
    let _ = owner;
    Some(Rc::new(macos::MacFileDialogs))
  }
  #[cfg(not(any(windows, target_os = "macos")))]
  {
    let _ = owner;
    crate::dialogs::find_program().map(|p| {
      Rc::new(crate::dialogs::ProgramDialogs(p))
        as Rc<dyn dovetail_core::FileDialogs>
    })
  }
}

/// The system's light or dark mode, accent colour and high contrast (M14):
/// from the registry and `SystemParametersInfo` on Windows, AppKit on
/// macOS, and GNOME's settings through `gsettings` elsewhere.
pub(crate) fn appearance() -> dovetail_core::SystemAppearance {
  #[cfg(windows)]
  {
    windows::appearance()
  }
  #[cfg(target_os = "macos")]
  {
    macos::appearance()
  }
  #[cfg(not(any(windows, target_os = "macos")))]
  {
    crate::appearance::from_gsettings()
  }
}

/// True when scrollbars show only while scrolling, as macOS does with its
/// usual setting (M15, D0021). Windows and Linux always show them.
pub(crate) fn overlay_scrollbars() -> bool {
  #[cfg(target_os = "macos")]
  {
    macos::overlay_scrollbars()
  }
  #[cfg(not(target_os = "macos"))]
  {
    false
  }
}

/// How long a text caret stays on, and then off: the system setting on
/// Windows, and 530 ms elsewhere. `None` when the user turned blinking off.
pub(crate) fn caret_blink_time() -> Option<std::time::Duration> {
  #[cfg(windows)]
  {
    windows::caret_blink_time()
  }
  #[cfg(not(windows))]
  {
    Some(std::time::Duration::from_millis(530))
  }
}

/// The system clipboard, or `None` when this system has none Dovetail can
/// use. Then the clipboard stays inside the application.
pub(crate) fn clipboard() -> Option<Rc<dyn Clipboard>> {
  #[cfg(windows)]
  {
    Some(Rc::new(windows::WindowsClipboard))
  }
  #[cfg(target_os = "macos")]
  {
    Some(Rc::new(macos::MacClipboard))
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    x11::X11Clipboard::new().map(|c| Rc::new(c) as Rc<dyn Clipboard>)
  }
  #[cfg(not(any(windows, unix)))]
  {
    None
  }
}

#[cfg(test)]
mod tests {
  /// Reading the system's language and blink time changes nothing, so it
  /// runs every time.
  #[test]
  fn the_system_language_and_blink_time_read() {
    let tag = super::language();
    println!("language: {tag:?}, blink: {:?}", super::caret_blink_time());
    #[cfg(windows)]
    {
      let tag = tag.expect("Windows has a display language");
      assert!(tag.len() >= 2 && tag.is_ascii(), "{tag}");
    }
  }

  /// Puts text on the real system clipboard and reads it back, then puts
  /// back what was there. Ignored by default, because it changes the
  /// clipboard of the person running the tests:
  /// `cargo test -p dovetail-desktop -- --ignored clipboard`.
  #[test]
  #[ignore = "changes the system clipboard"]
  fn the_system_clipboard_round_trips_text() {
    let clipboard = super::clipboard().expect("a system clipboard");
    let before = clipboard.text();
    let text = "Dovetail \u{2713} 中文\nsecond line";
    assert!(clipboard.set_text(text));
    let back = clipboard.text().map(|t| t.replace("\r\n", "\n"));
    if let Some(before) = before {
      clipboard.set_text(&before);
    }
    assert_eq!(back.as_deref(), Some(text));
  }
}
