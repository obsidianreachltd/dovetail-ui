//! Platform code that winit, wgpu and AccessKit do not provide. The only
//! place in Dovetail where `unsafe` is allowed (D0013): each module below
//! wraps one operating system's interface and offers a safe one.

use std::rc::Rc;

use dovetail_core::Clipboard;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_windows;
#[cfg(windows)]
mod windows;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11_windows;

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

/// Less motion, the text scale, the caret blink, double-clicks and the
/// scroll bar style (M18, D0028): from `SystemParametersInfo`, the
/// registry and user32 on Windows, AppKit on macOS, and GNOME's settings
/// through `gsettings` elsewhere.
pub(crate) fn settings() -> dovetail_core::SystemSettings {
  #[cfg(windows)]
  {
    windows::settings()
  }
  #[cfg(target_os = "macos")]
  {
    macos::settings()
  }
  #[cfg(not(any(windows, target_os = "macos")))]
  {
    crate::system_settings::from_gsettings()
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

/// The work area of a monitor (M19): the monitor without the taskbar, the
/// menu bar or the Dock, in physical pixels, from `GetMonitorInfoW` on
/// Windows, `NSScreen` on macOS and the window manager on X11. `None`
/// where the system does not say, as on Wayland.
pub(crate) fn work_area(
  monitor: &winit::monitor::MonitorHandle
) -> Option<dovetail_core::ScreenRect> {
  #[cfg(windows)]
  {
    use winit::platform::windows::MonitorHandleExtWindows;
    windows::work_area(monitor.hmonitor())
  }
  #[cfg(target_os = "macos")]
  {
    use winit::platform::macos::MonitorHandleExtMacOS;
    let [left, top, right, bottom] =
      macos_windows::work_area_insets(monitor.ns_screen()?)?;
    let scale = monitor.scale_factor();
    let (p, s) = (monitor.position(), monitor.size());
    Some(dovetail_core::ScreenRect::new(
      (f64::from(p.x) + left * scale) as f32,
      (f64::from(p.y) + top * scale) as f32,
      (f64::from(s.width) - (left + right) * scale) as f32,
      (f64::from(s.height) - (top + bottom) * scale) as f32,
    ))
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    let (p, s) = (monitor.position(), monitor.size());
    x11_windows::work_area(dovetail_core::ScreenRect::new(
      p.x as f32,
      p.y as f32,
      s.width as f32,
      s.height as f32,
    ))
  }
  #[cfg(not(any(windows, unix)))]
  {
    let _ = monitor;
    None
  }
}

/// Tells the system that `child` belongs to `owner`, so it stays in front
/// of it (M19): a child window on macOS, `WM_TRANSIENT_FOR` on X11. On
/// Windows the owner is given when the window is made, and on Wayland
/// nothing is done.
pub(crate) fn make_owned(
  child: &winit::window::Window,
  owner: &winit::window::Window,
) {
  use winit::raw_window_handle::HasWindowHandle;
  #[cfg(unix)]
  use winit::raw_window_handle::RawWindowHandle;
  let raw =
    |w: &winit::window::Window| w.window_handle().ok().map(|h| h.as_raw());
  match (raw(child), raw(owner)) {
    #[cfg(target_os = "macos")]
    (Some(RawWindowHandle::AppKit(c)), Some(RawWindowHandle::AppKit(o))) => {
      macos_windows::add_child_window(o.ns_view.as_ptr(), c.ns_view.as_ptr());
    },
    #[cfg(all(unix, not(target_os = "macos")))]
    (Some(RawWindowHandle::Xlib(c)), Some(RawWindowHandle::Xlib(o))) => {
      x11_windows::set_transient_for(c.window, o.window);
    },
    #[cfg(all(unix, not(target_os = "macos")))]
    (Some(RawWindowHandle::Xcb(c)), Some(RawWindowHandle::Xcb(o))) => {
      x11_windows::set_transient_for(
        std::ffi::c_ulong::from(c.window.get()),
        std::ffi::c_ulong::from(o.window.get()),
      );
    },
    _ => {},
  }
}

/// Calls `reopen` when the user clicks the application's Dock icon while
/// no window is open (M19). Only macOS has one.
pub(crate) fn on_reopen(reopen: Box<dyn Fn()>) {
  #[cfg(target_os = "macos")]
  {
    macos_windows::on_reopen(reopen);
  }
  #[cfg(not(target_os = "macos"))]
  {
    let _ = reopen;
  }
}

#[cfg(test)]
mod tests {
  /// Reading the system's language and settings changes nothing, so it
  /// runs every time.
  #[test]
  fn the_system_language_and_settings_read() {
    let tag = super::language();
    println!("language: {tag:?}");
    println!("settings: {:?}", super::settings());
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
