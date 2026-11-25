//! The Windows clipboard, through the Win32 clipboard functions in
//! `user32.dll` and the global memory functions in `kernel32.dll`, and the
//! caret blink time and display language the user chose (D0013). Text is exchanged as `CF_UNICODETEXT`: UTF-16 ending in a zero.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::ptr;
use std::thread;
use std::time::Duration;

use dovetail_core::Clipboard;

type Handle = *mut c_void;

const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;

#[link(name = "user32")]
unsafe extern "system" {
  fn OpenClipboard(owner: Handle) -> i32;
  fn CloseClipboard() -> i32;
  fn EmptyClipboard() -> i32;
  fn GetClipboardData(format: u32) -> Handle;
  fn SetClipboardData(
    format: u32,
    memory: Handle,
  ) -> Handle;
  fn GetCaretBlinkTime() -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
  fn GlobalAlloc(
    flags: u32,
    bytes: usize,
  ) -> Handle;
  fn GlobalFree(memory: Handle) -> Handle;
  fn GlobalLock(memory: Handle) -> *mut c_void;
  fn GlobalUnlock(memory: Handle) -> i32;
  fn GlobalSize(memory: Handle) -> usize;
  fn GetUserDefaultUILanguage() -> u16;
  fn LCIDToLocaleName(
    locale: u32,
    name: *mut u16,
    length: i32,
    flags: u32,
  ) -> i32;
}

/// The clipboard while it is open. Closing it when dropped means every
/// way out of a function closes it.
struct Open;

impl Open {
  /// Opens the clipboard. Another program may have it open for a moment,
  /// so this tries a few times.
  fn new() -> Option<Open> {
    for _ in 0..10 {
      // SAFETY: OpenClipboard takes an optional window handle; null means
      // the clipboard is opened for this task. It has no other inputs.
      if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
        return Some(Open);
      }
      thread::sleep(Duration::from_millis(5));
    }
    None
  }
}

impl Drop for Open {
  fn drop(&mut self) {
    // SAFETY: an `Open` exists only while this thread has the clipboard
    // open, and it is dropped once.
    unsafe {
      CloseClipboard();
    }
  }
}

/// The system clipboard on Windows.
pub struct WindowsClipboard;

impl Clipboard for WindowsClipboard {
  fn text(&self) -> Option<String> {
    let _open = Open::new()?;
    // SAFETY: the clipboard is open (`_open`). The handle is owned by the
    // clipboard and stays valid until it is closed; it is only read.
    let memory = unsafe { GetClipboardData(CF_UNICODETEXT) };
    if memory.is_null() {
      return None;
    }
    // SAFETY: `memory` is a global memory handle from the clipboard.
    // GlobalSize reads its size, and GlobalLock gives a pointer to that
    // many bytes, valid until GlobalUnlock.
    unsafe {
      let bytes = GlobalSize(memory);
      let data = GlobalLock(memory) as *const u16;
      if data.is_null() {
        return None;
      }
      let units = std::slice::from_raw_parts(data, bytes / 2);
      let end = units
        .iter()
        .position(|&u| u == 0)
        .unwrap_or(units.len());
      let text = String::from_utf16_lossy(&units[..end]);
      GlobalUnlock(memory);
      Some(text)
    }
  }

  fn set_text(
    &self,
    text: &str,
  ) -> bool {
    let units: Vec<u16> = text
      .replace('\n', "\r\n")
      .encode_utf16()
      .chain([0])
      .collect();
    let Some(_open) = Open::new() else {
      return false;
    };
    // SAFETY: the clipboard is open (`_open`). The memory is allocated
    // with room for every unit, filled while locked, and unlocked before
    // it is handed over. When SetClipboardData succeeds the system owns
    // the memory; when anything fails it is freed here, once.
    unsafe {
      if EmptyClipboard() == 0 {
        return false;
      }
      let memory = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
      if memory.is_null() {
        return false;
      }
      let data = GlobalLock(memory) as *mut u16;
      if data.is_null() {
        GlobalFree(memory);
        return false;
      }
      ptr::copy_nonoverlapping(units.as_ptr(), data, units.len());
      GlobalUnlock(memory);
      if SetClipboardData(CF_UNICODETEXT, memory).is_null() {
        GlobalFree(memory);
        return false;
      }
    }
    true
  }
}

/// How long the caret stays on, and then off, as set in Windows; `None`
/// when the user turned blinking off.
pub fn caret_blink_time() -> Option<std::time::Duration> {
  // SAFETY: GetCaretBlinkTime takes nothing and returns milliseconds, or
  // INFINITE (u32::MAX) when the caret does not blink, or 0 on failure.
  let ms = unsafe { GetCaretBlinkTime() };
  match ms {
    u32::MAX => None,
    0 => Some(std::time::Duration::from_millis(530)),
    ms => Some(std::time::Duration::from_millis(u64::from(ms))),
  }
}

/// The language Windows shows its own text in, as a tag such as `ja-JP`.
pub fn language() -> Option<String> {
  // LOCALE_NAME_MAX_LENGTH.
  let mut name = [0u16; 85];
  // SAFETY: GetUserDefaultUILanguage takes nothing. LCIDToLocaleName
  // writes at most `name.len()` units, ending in a zero, and returns how
  // many it wrote (with the zero), or 0 on failure.
  let written = unsafe {
    let id = GetUserDefaultUILanguage();
    LCIDToLocaleName(u32::from(id), name.as_mut_ptr(), name.len() as i32, 0)
  };
  let len = usize::try_from(written).ok()?.checked_sub(1)?;
  Some(String::from_utf16_lossy(name.get(..len)?))
}

/// `OPENFILENAMEW` from `commdlg.h`.
#[repr(C)]
struct OpenFileName {
  struct_size: u32,
  owner: Handle,
  instance: Handle,
  filter: *const u16,
  custom_filter: *mut u16,
  max_custom_filter: u32,
  filter_index: u32,
  file: *mut u16,
  max_file: u32,
  file_title: *mut u16,
  max_file_title: u32,
  initial_dir: *const u16,
  title: *const u16,
  flags: u32,
  file_offset: u16,
  file_extension: u16,
  default_ext: *const u16,
  cust_data: isize,
  hook: *const c_void,
  template_name: *const u16,
  reserved_ptr: *mut c_void,
  reserved: u32,
  flags_ex: u32,
}

const OFN_OVERWRITEPROMPT: u32 = 0x0000_0002;
const OFN_HIDEREADONLY: u32 = 0x0000_0004;
const OFN_NOCHANGEDIR: u32 = 0x0000_0008;
const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
const OFN_EXPLORER: u32 = 0x0008_0000;

#[link(name = "comdlg32")]
unsafe extern "system" {
  fn GetOpenFileNameW(dialog: *mut OpenFileName) -> i32;
  fn GetSaveFileNameW(dialog: *mut OpenFileName) -> i32;
}

/// UTF-16 ending in a zero.
fn wide(text: &str) -> Vec<u16> {
  text.encode_utf16().chain([0]).collect()
}

/// The filter list the dialog wants: pairs of a name and patterns, each
/// ending in a zero, and a zero after the last pair.
fn filter_list(filters: &[dovetail_core::FileFilter]) -> Vec<u16> {
  let mut list = Vec::new();
  for f in filters {
    let patterns = if f.extensions.is_empty() {
      "*.*".to_owned()
    } else {
      f.extensions
        .iter()
        .map(|e| format!("*.{e}"))
        .collect::<Vec<_>>()
        .join(";")
    };
    list.extend(wide(&format!("{} ({patterns})", f.name)));
    list.extend(wide(&patterns));
  }
  list.push(0);
  list
}

/// The system's open and save dialogs (M11, D0017), owned by the window
/// in front, whose handle `owner` holds.
pub struct WindowsFileDialogs {
  pub owner: std::rc::Rc<std::cell::Cell<isize>>,
}

impl dovetail_core::FileDialogs for WindowsFileDialogs {
  fn show(
    &self,
    dialog: dovetail_core::FileDialog,
    done: dovetail_core::Callback<Option<std::path::PathBuf>>,
  ) -> bool {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let open = dialog.kind == dovetail_core::FileDialogKind::Open;
    // Long paths fit: 32,767 characters and the zero.
    let mut file = vec![0u16; 32_768];
    if let Some(name) = &dialog.name {
      let name: Vec<u16> = name.encode_utf16().take(file.len() - 1).collect();
      file[..name.len()].copy_from_slice(&name);
    }
    let filter =
      (!dialog.filters.is_empty()).then(|| filter_list(&dialog.filters));
    let title = wide(&dialog.title);
    let folder: Option<Vec<u16>> = dialog
      .folder
      .as_ref()
      .map(|f| f.as_os_str().encode_wide().chain([0]).collect());
    let default_ext = dialog.default_extension().map(wide);
    let mut flags =
      OFN_EXPLORER | OFN_NOCHANGEDIR | OFN_HIDEREADONLY | OFN_PATHMUSTEXIST;
    flags |= if open {
      OFN_FILEMUSTEXIST
    } else {
      OFN_OVERWRITEPROMPT
    };
    let mut info = OpenFileName {
      struct_size: std::mem::size_of::<OpenFileName>() as u32,
      owner: self.owner.get() as Handle,
      instance: ptr::null_mut(),
      filter: filter
        .as_ref()
        .map_or(ptr::null(), |f| f.as_ptr()),
      custom_filter: ptr::null_mut(),
      max_custom_filter: 0,
      filter_index: 1,
      file: file.as_mut_ptr(),
      max_file: file.len() as u32,
      file_title: ptr::null_mut(),
      max_file_title: 0,
      initial_dir: folder
        .as_ref()
        .map_or(ptr::null(), |f| f.as_ptr()),
      title: title.as_ptr(),
      flags,
      file_offset: 0,
      file_extension: 0,
      default_ext: default_ext
        .as_ref()
        .map_or(ptr::null(), |e| e.as_ptr()),
      cust_data: 0,
      hook: ptr::null(),
      template_name: ptr::null(),
      reserved_ptr: ptr::null_mut(),
      reserved: 0,
      flags_ex: 0,
    };
    // SAFETY: `info` is a correctly sized OPENFILENAMEW. Every pointer in
    // it points into a buffer owned by this function (`file`, `filter`,
    // `title`, `folder`, `default_ext`), each ending in the zeros the
    // dialog needs, and all outlive the call. `max_file` is `file`'s
    // length, so the dialog writes inside it. The owner is a window
    // handle, or null; a stale handle only makes the dialog unowned.
    let chosen = unsafe {
      if open {
        GetOpenFileNameW(&mut info)
      } else {
        GetSaveFileNameW(&mut info)
      }
    } != 0;
    let path = chosen.then(|| {
      let end = file
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(file.len());
      std::path::PathBuf::from(std::ffi::OsString::from_wide(&file[..end]))
    });
    done.call(path.filter(|p| !p.as_os_str().is_empty()));
    true
  }
}

/// `HKEY_CURRENT_USER`.
const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
/// `RRF_RT_REG_DWORD`: only a 32-bit value.
const RRF_RT_REG_DWORD: u32 = 0x0000_0010;
/// `SPI_GETHIGHCONTRAST`.
const SPI_GETHIGHCONTRAST: u32 = 0x0042;
/// `HCF_HIGHCONTRASTON`.
const HCF_HIGHCONTRASTON: u32 = 0x0000_0001;

/// `HIGHCONTRASTW`.
#[repr(C)]
struct HighContrast {
  size: u32,
  flags: u32,
  scheme: *mut u16,
}

#[link(name = "advapi32")]
unsafe extern "system" {
  fn RegGetValueW(
    key: isize,
    subkey: *const u16,
    value: *const u16,
    flags: u32,
    kind: *mut u32,
    data: *mut c_void,
    size: *mut u32,
  ) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
  fn SystemParametersInfoW(
    action: u32,
    param: u32,
    data: *mut c_void,
    win_ini: u32,
  ) -> i32;
}

/// A 32-bit value under `HKEY_CURRENT_USER`, if it is there.
fn user_dword(
  subkey: &str,
  value: &str,
) -> Option<u32> {
  let subkey = wide(subkey);
  let value = wide(value);
  let mut data: u32 = 0;
  let mut size = std::mem::size_of::<u32>() as u32;
  // SAFETY: both names end in a zero and live through the call; `data`
  // is a u32 and `size` says so, and the flag limits the value to a
  // 32-bit one, so the call writes at most four bytes into it.
  let status = unsafe {
    RegGetValueW(
      HKEY_CURRENT_USER,
      subkey.as_ptr(),
      value.as_ptr(),
      RRF_RT_REG_DWORD,
      ptr::null_mut(),
      (&mut data as *mut u32).cast(),
      &mut size,
    )
  };
  (status == 0).then_some(data)
}

/// Light or dark from the user's app mode, the accent colour from the
/// window manager's setting, and high contrast (M14).
pub fn appearance() -> dovetail_core::SystemAppearance {
  let personalize =
    r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
  let dark = user_dword(personalize, "AppsUseLightTheme") == Some(0);
  // 0xAABBGGRR.
  let accent = user_dword(r"Software\Microsoft\Windows\DWM", "AccentColor")
    .map(|abgr| {
      let channel =
        |shift: u32| f32::from(((abgr >> shift) & 0xff) as u8) / 255.0;
      dovetail_core::Color::rgb(channel(0), channel(8), channel(16))
    });
  let mut contrast = HighContrast {
    size: std::mem::size_of::<HighContrast>() as u32,
    flags: 0,
    scheme: ptr::null_mut(),
  };
  // SAFETY: `contrast` is a HIGHCONTRASTW with its size set, as
  // SPI_GETHIGHCONTRAST needs, and lives through the call. The scheme name
  // it points to belongs to the system and is not read.
  let read = unsafe {
    SystemParametersInfoW(
      SPI_GETHIGHCONTRAST,
      contrast.size,
      (&mut contrast as *mut HighContrast).cast(),
      0,
    )
  } != 0;
  dovetail_core::SystemAppearance {
    dark,
    accent,
    high_contrast: read && contrast.flags & HCF_HIGHCONTRASTON != 0,
  }
}
