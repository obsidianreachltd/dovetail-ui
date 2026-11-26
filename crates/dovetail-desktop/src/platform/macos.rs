//! macOS platform code, through the Objective-C runtime in `libobjc`
//! (D0013):
//!
//! - the clipboard: AppKit's general `NSPasteboard`, with text exchanged as
//!   `NSPasteboardTypeString`;
//! - the user's preferred language, from CoreFoundation;
//! - the menu bar (M08): `NSMenu`s built from a `MenuModel`, whose items
//!   call back into the application through a small class made at run
//!   time, with commands' icons as `NSImage`s (M17).
#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_void};

use std::cell::RefCell;

use dovetail_core::{
  Callback, Clipboard, MenuIcon, MenuItemSpec, MenuKey, MenuModel, NativeMenus,
};

type Id = *mut c_void;
type Sel = *const c_void;

/// `NSUTF8StringEncoding`.
const UTF8: usize = 4;

#[link(name = "objc")]
unsafe extern "C" {
  fn objc_getClass(name: *const c_char) -> Id;
  fn sel_registerName(name: *const c_char) -> Sel;
  fn objc_msgSend();
  fn objc_autoreleasePoolPush() -> *mut c_void;
  fn objc_autoreleasePoolPop(pool: *mut c_void);
  fn objc_allocateClassPair(
    superclass: Id,
    name: *const c_char,
    extra_bytes: usize,
  ) -> Id;
  fn objc_registerClassPair(class: Id);
  fn class_addMethod(
    class: Id,
    name: Sel,
    imp: *const c_void,
    types: *const c_char,
  ) -> i8;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
  fn CFLocaleCopyPreferredLanguages() -> *const c_void;
  fn CFArrayGetCount(array: *const c_void) -> isize;
  fn CFArrayGetValueAtIndex(
    array: *const c_void,
    index: isize,
  ) -> *const c_void;
  fn CFStringGetCString(
    string: *const c_void,
    buffer: *mut c_char,
    size: isize,
    encoding: u32,
  ) -> u8;
  fn CFRelease(object: *const c_void);
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
  static NSPasteboardTypeString: Id;
  static NSDeviceRGBColorSpace: Id;
}

/// `NSSize`: a width and height in points.
#[repr(C)]
#[derive(Clone, Copy)]
struct NsSize {
  width: f64,
  height: f64,
}

/// An `NSImage` of a menu icon (M17), which the caller owns and releases
/// once, or null if AppKit could not make one.
fn ns_image(icon: &MenuIcon) -> Id {
  let (rep_class, image_class) =
    (class(c"NSBitmapImageRep"), class(c"NSImage"));
  let bytes = icon.width as usize * icon.height as usize * 4;
  if rep_class.is_null() || image_class.is_null() || icon.pixels.len() != bytes
  {
    return std::ptr::null_mut();
  }
  // SAFETY: each objc_msgSend cast matches its method:
  // - `+alloc` takes nothing and returns an object;
  // - `-initWithBitmapDataPlanes:pixelsWide:pixelsHigh:bitsPerSample:
  //   samplesPerPixel:hasAlpha:isPlanar:colorSpaceName:bytesPerRow:
  //   bitsPerPixel:` takes a pointer to plane pointers (null, so the rep
  //   allocates its own buffer), five NSIntegers and BOOLs as listed, an
  //   NSString and two NSIntegers, and returns the rep or nil;
  // - `-bitmapData` takes nothing and returns the rep's buffer, which holds
  //   `bytesPerRow × pixelsHigh` bytes: exactly `bytes`, checked above, so
  //   the copy stays inside it;
  // - `-initWithSize:` takes an NSSize (two CGFloats, f64 on 64-bit
  //   systems) and returns an image;
  // - `-addRepresentation:` takes an object, which the image keeps, so the
  //   rep is released once after; `-setTemplate:` takes a BOOL.
  unsafe {
    let send: unsafe extern "C" fn(Id, Sel) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let send_id: unsafe extern "C" fn(Id, Sel, Id) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let send_bool: unsafe extern "C" fn(Id, Sel, i8) =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let init_rep: unsafe extern "C" fn(
      Id,
      Sel,
      *mut *mut u8,
      isize,
      isize,
      isize,
      isize,
      i8,
      i8,
      Id,
      isize,
      isize,
    ) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let data: unsafe extern "C" fn(Id, Sel) -> *mut u8 =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let init_size: unsafe extern "C" fn(Id, Sel, NsSize) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());

    let rep = init_rep(
      send(rep_class, sel(c"alloc")),
      sel(c"initWithBitmapDataPlanes:pixelsWide:pixelsHigh:bitsPerSample:samplesPerPixel:hasAlpha:isPlanar:colorSpaceName:bytesPerRow:bitsPerPixel:"),
      std::ptr::null_mut(),
      icon.width as isize,
      icon.height as isize,
      8,
      4,
      1,
      0,
      NSDeviceRGBColorSpace,
      icon.width as isize * 4,
      32,
    );
    if rep.is_null() {
      return std::ptr::null_mut();
    }
    let buffer = data(rep, sel(c"bitmapData"));
    if buffer.is_null() {
      send(rep, sel(c"release"));
      return std::ptr::null_mut();
    }
    std::ptr::copy_nonoverlapping(icon.pixels.as_ptr(), buffer, bytes);
    let size = NsSize {
      width: f64::from(icon.size),
      height: f64::from(icon.size),
    };
    let image = init_size(
      send(image_class, sel(c"alloc")),
      sel(c"initWithSize:"),
      size,
    );
    if !image.is_null() {
      send_id(image, sel(c"addRepresentation:"), rep);
      send_bool(image, sel(c"setTemplate:"), i8::from(icon.template));
    }
    send(rep, sel(c"release"));
    image
  }
}

/// An autorelease pool for the length of one clipboard call, so objects
/// AppKit returns autoreleased are freed when it ends.
struct Pool(*mut c_void);

impl Pool {
  fn new() -> Pool {
    // SAFETY: pushing a pool has no inputs; it is popped once, on drop,
    // on the same thread.
    Pool(unsafe { objc_autoreleasePoolPush() })
  }
}

impl Drop for Pool {
  fn drop(&mut self) {
    // SAFETY: `self.0` came from objc_autoreleasePoolPush on this thread
    // and is popped once.
    unsafe { objc_autoreleasePoolPop(self.0) }
  }
}

fn class(name: &CStr) -> Id {
  // SAFETY: the name is a valid C string; an unknown class gives null.
  unsafe { objc_getClass(name.as_ptr()) }
}

fn sel(name: &CStr) -> Sel {
  // SAFETY: the name is a valid C string; selectors live for the process.
  unsafe { sel_registerName(name.as_ptr()) }
}

/// The general pasteboard, or null.
fn pasteboard() -> Id {
  let class = class(c"NSPasteboard");
  if class.is_null() {
    return std::ptr::null_mut();
  }
  // SAFETY: `+[NSPasteboard generalPasteboard]` takes no arguments and
  // returns an object; objc_msgSend is cast to exactly that signature.
  unsafe {
    let send: unsafe extern "C" fn(Id, Sel) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    send(class, sel(c"generalPasteboard"))
  }
}

/// The system clipboard on macOS.
pub struct MacClipboard;

impl Clipboard for MacClipboard {
  fn text(&self) -> Option<String> {
    let _pool = Pool::new();
    let board = pasteboard();
    if board.is_null() {
      return None;
    }
    // SAFETY: `-[NSPasteboard stringForType:]` takes one object and returns
    // an NSString or nil; `-[NSString UTF8String]` takes nothing and
    // returns a C string that lives as long as the pool (`_pool`), which
    // outlives the copy made here. Each objc_msgSend cast matches the
    // method's signature.
    unsafe {
      let send_id: unsafe extern "C" fn(Id, Sel, Id) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let string =
        send_id(board, sel(c"stringForType:"), NSPasteboardTypeString);
      if string.is_null() {
        return None;
      }
      let send_str: unsafe extern "C" fn(Id, Sel) -> *const c_char =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let utf8 = send_str(string, sel(c"UTF8String"));
      if utf8.is_null() {
        return None;
      }
      Some(
        CStr::from_ptr(utf8)
          .to_string_lossy()
          .into_owned(),
      )
    }
  }

  fn set_text(
    &self,
    text: &str,
  ) -> bool {
    let _pool = Pool::new();
    let board = pasteboard();
    let string_class = class(c"NSString");
    if board.is_null() || string_class.is_null() {
      return false;
    }
    // SAFETY: each objc_msgSend cast matches its method's signature:
    // `alloc` and `clearContents` take nothing; `initWithBytes:length:
    // encoding:` takes a pointer to `text`'s bytes (read during the call
    // only), their length and an encoding; `setString:forType:` takes two
    // objects and returns a BOOL. The string made with alloc/init is owned
    // here and released once.
    unsafe {
      let send: unsafe extern "C" fn(Id, Sel) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let init: unsafe extern "C" fn(
        Id,
        Sel,
        *const c_void,
        usize,
        usize,
      ) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let set: unsafe extern "C" fn(Id, Sel, Id, Id) -> i8 =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());

      let string = init(
        send(string_class, sel(c"alloc")),
        sel(c"initWithBytes:length:encoding:"),
        text.as_ptr().cast(),
        text.len(),
        UTF8,
      );
      if string.is_null() {
        return false;
      }
      send(board, sel(c"clearContents"));
      let done = set(
        board,
        sel(c"setString:forType:"),
        string,
        NSPasteboardTypeString,
      );
      send(string, sel(c"release"));
      done != 0
    }
  }
}

/// The user's first preferred language, as a tag such as `ja-JP`.
pub fn language() -> Option<String> {
  /// `kCFStringEncodingUTF8`.
  const UTF8_ENCODING: u32 = 0x0800_0100;
  // SAFETY: CFLocaleCopyPreferredLanguages returns an array this code
  // owns, released once below. Its first item is a CFString owned by the
  // array, read while the array is alive. CFStringGetCString writes at
  // most `buffer.len()` bytes, ending in a zero, and returns false when
  // they do not fit.
  unsafe {
    let languages = CFLocaleCopyPreferredLanguages();
    if languages.is_null() {
      return None;
    }
    let mut buffer = [0 as c_char; 64];
    let found = CFArrayGetCount(languages) > 0 && {
      let first = CFArrayGetValueAtIndex(languages, 0);
      !first.is_null()
        && CFStringGetCString(
          first,
          buffer.as_mut_ptr(),
          buffer.len() as isize,
          UTF8_ENCODING,
        ) != 0
    };
    CFRelease(languages);
    found.then(|| {
      CStr::from_ptr(buffer.as_ptr())
        .to_string_lossy()
        .into_owned()
    })
  }
}

// --- The menu bar (M08) -------------------------------------------------------

/// `NSEventModifierFlagShift`, `...Option` and `...Command`.
const SHIFT: usize = 1 << 17;
const OPTION: usize = 1 << 19;
const COMMAND: usize = 1 << 20;

thread_local! {
  /// The command id of each menu item, by its tag, and who to tell when one
  /// is chosen. Menus are only used on the main thread.
  static CHOICES: RefCell<(Vec<String>, Option<Callback<String>>)> =
    const { RefCell::new((Vec::new(), None)) };
  /// The object every menu item sends its action to.
  static TARGET: Id = make_target();
}

/// Called by AppKit when a menu item made here is chosen.
extern "C" fn chosen(
  _this: Id,
  _cmd: Sel,
  sender: Id,
) {
  // SAFETY: `sender` is the NSMenuItem that was chosen; `-tag` takes no
  // arguments and returns an NSInteger.
  let tag = unsafe {
    let send: unsafe extern "C" fn(Id, Sel) -> isize =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    send(sender, sel(c"tag"))
  };
  let choice = CHOICES.with_borrow(|(ids, call)| {
    let id = usize::try_from(tag)
      .ok()
      .and_then(|t| ids.get(t))?;
    Some((id.clone(), call.clone()?))
  });
  // The command runs outside the borrow, as it may change the menus.
  if let Some((id, call)) = choice {
    call.call(id);
  }
}

/// An instance of `DovetailMenuTarget`, a subclass of NSObject with one
/// method, `dovetailChoose:`, that calls [`chosen`].
fn make_target() -> Id {
  let name = c"DovetailMenuTarget";
  // SAFETY: the class is made once per process: if a class of that name
  // exists it is used, otherwise it is made from NSObject with one method
  // whose type string ("v@:@": returns nothing, takes self, _cmd and an
  // object) matches `chosen`, and registered before use. `alloc` and
  // `init` take no arguments; the object is kept for the life of the
  // thread.
  unsafe {
    let mut target_class =
      objc_allocateClassPair(class(c"NSObject"), name.as_ptr(), 0);
    if target_class.is_null() {
      target_class = objc_getClass(name.as_ptr());
    } else {
      let imp: extern "C" fn(Id, Sel, Id) = chosen;
      class_addMethod(
        target_class,
        sel(c"dovetailChoose:"),
        imp as *const c_void,
        c"v@:@".as_ptr(),
      );
      objc_registerClassPair(target_class);
    }
    let send: unsafe extern "C" fn(Id, Sel) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    send(send(target_class, sel(c"alloc")), sel(c"init"))
  }
}

/// A new NSString holding `text`, owned by the caller, who releases it.
fn ns_string(text: &str) -> Id {
  // SAFETY: as in `MacClipboard::set_text`: alloc, then
  // initWithBytes:length:encoding: with a pointer to `text`'s bytes, read
  // during the call only.
  unsafe {
    let send: unsafe extern "C" fn(Id, Sel) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let init: unsafe extern "C" fn(Id, Sel, *const c_void, usize, usize) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    init(
      send(class(c"NSString"), sel(c"alloc")),
      sel(c"initWithBytes:length:encoding:"),
      text.as_ptr().cast(),
      text.len(),
      UTF8,
    )
  }
}

/// The key equivalent AppKit wants for a shortcut's key: the character
/// itself, or the function-key character for a named key.
fn key_equivalent(key: &MenuKey) -> Option<String> {
  let named = |code: u32| char::from_u32(code).map(String::from);
  let mut chars = key.key.chars();
  if let (Some(c), None) = (chars.next(), chars.next()) {
    return Some(c.to_lowercase().collect());
  }
  match key.key.as_str() {
    "Enter" => Some("\r".into()),
    "Esc" => Some("\u{1b}".into()),
    "Tab" => Some("\t".into()),
    "Space" => Some(" ".into()),
    "Backspace" => Some("\u{8}".into()),
    "Delete" => named(0xF728),
    "Insert" => named(0xF727),
    "Up" => named(0xF700),
    "Down" => named(0xF701),
    "Left" => named(0xF702),
    "Right" => named(0xF703),
    "Home" => named(0xF729),
    "End" => named(0xF72B),
    "PageUp" => named(0xF72C),
    "PageDown" => named(0xF72D),
    f if f.starts_with('F') => {
      let n: u32 = f[1..].parse().ok()?;
      (1..=12)
        .contains(&n)
        .then(|| named(0xF703 + n))
        .flatten()
    },
    _ => None,
  }
}

/// The macOS menu bar.
pub struct MacMenus;

impl NativeMenus for MacMenus {
  fn set_menus(
    &self,
    menus: &MenuModel,
    on_choose: Callback<String>,
  ) {
    let _pool = Pool::new();
    let target = TARGET.with(|t| *t);
    let mut ids = Vec::new();
    // SAFETY: every objc_msgSend cast below matches its method:
    // - `+sharedApplication`, `-init`, `-alloc`, `+separatorItem`,
    //   `+processInfo` and `-processName` take nothing and return an object;
    // - `-initWithTitle:` takes an NSString; `-initWithTitle:action:
    //   keyEquivalent:` takes an NSString, a selector (or null) and an
    //   NSString;
    // - `-addItem:`, `-setSubmenu:`, `-setTarget:`, `-setMainMenu:` take an
    //   object; `-setTag:` an NSInteger; `-setKeyEquivalentModifierMask:` an
    //   NSUInteger; `-setEnabled:` and `-setAutoenablesItems:` a BOOL.
    // Objects made with alloc/init are released once, after they are added
    // to a menu, which keeps them; strings likewise after use. The pool
    // (`_pool`) frees what AppKit returns autoreleased.
    unsafe {
      let send: unsafe extern "C" fn(Id, Sel) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let send_id: unsafe extern "C" fn(Id, Sel, Id) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let send_item: unsafe extern "C" fn(Id, Sel, Id, Sel, Id) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let send_int: unsafe extern "C" fn(Id, Sel, isize) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let send_uint: unsafe extern "C" fn(Id, Sel, usize) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let send_bool: unsafe extern "C" fn(Id, Sel, i8) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());

      let app = send(class(c"NSApplication"), sel(c"sharedApplication"));
      let new_menu = |title: &str| {
        let t = ns_string(title);
        let menu = send_id(
          send(class(c"NSMenu"), sel(c"alloc")),
          sel(c"initWithTitle:"),
          t,
        );
        send(t, sel(c"release"));
        send_bool(menu, sel(c"setAutoenablesItems:"), 0);
        menu
      };
      let new_item = |title: &str, action: Sel, key: &str| {
        let (t, k) = (ns_string(title), ns_string(key));
        let item = send_item(
          send(class(c"NSMenuItem"), sel(c"alloc")),
          sel(c"initWithTitle:action:keyEquivalent:"),
          t,
          action,
          k,
        );
        send(t, sel(c"release"));
        send(k, sel(c"release"));
        item
      };
      // Adds `item` to `menu`, which keeps it.
      let add = |menu: Id, item: Id| {
        send_id(menu, sel(c"addItem:"), item);
        send(item, sel(c"release"));
      };

      let bar = new_menu("");
      // The application menu, first, with Quit.
      let info = send(class(c"NSProcessInfo"), sel(c"processInfo"));
      let name_ns = send(info, sel(c"processName"));
      let utf8: unsafe extern "C" fn(Id, Sel) -> *const c_char =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let name = if name_ns.is_null() {
        String::from("Application")
      } else {
        CStr::from_ptr(utf8(name_ns, sel(c"UTF8String")))
          .to_string_lossy()
          .into_owned()
      };
      let app_item = new_item(&name, std::ptr::null(), "");
      let app_menu = new_menu(&name);
      let quit = new_item(&format!("Quit {name}"), sel(c"terminate:"), "q");
      send_uint(quit, sel(c"setKeyEquivalentModifierMask:"), COMMAND);
      add(app_menu, quit);
      send_id(app_item, sel(c"setSubmenu:"), app_menu);
      send(app_menu, sel(c"release"));
      add(bar, app_item);

      for spec in &menus.menus {
        let top = new_item(&spec.title, std::ptr::null(), "");
        let menu = new_menu(&spec.title);
        for item in &spec.items {
          match item {
            MenuItemSpec::Separator => {
              // Owned by the class, not by us: added without releasing.
              let line = send(class(c"NSMenuItem"), sel(c"separatorItem"));
              send_id(menu, sel(c"addItem:"), line);
            },
            MenuItemSpec::Command { id, label, enabled, key, icon } => {
              let equivalent = key
                .as_ref()
                .and_then(key_equivalent)
                .unwrap_or_default();
              let entry = new_item(label, sel(c"dovetailChoose:"), &equivalent);
              if let Some(key) = key {
                let mut mask = 0;
                if key.primary {
                  mask |= COMMAND;
                }
                if key.shift {
                  mask |= SHIFT;
                }
                if key.alt {
                  mask |= OPTION;
                }
                send_uint(entry, sel(c"setKeyEquivalentModifierMask:"), mask);
              }
              send_id(entry, sel(c"setTarget:"), target);
              send_int(entry, sel(c"setTag:"), ids.len() as isize);
              send_bool(entry, sel(c"setEnabled:"), i8::from(*enabled));
              // The item keeps its image: released once after.
              if let Some(image) = icon
                .as_ref()
                .map(ns_image)
                .filter(|i| !i.is_null())
              {
                send_id(entry, sel(c"setImage:"), image);
                send(image, sel(c"release"));
              }
              ids.push(id.clone());
              add(menu, entry);
            },
          }
        }
        send_id(top, sel(c"setSubmenu:"), menu);
        send(menu, sel(c"release"));
        add(bar, top);
      }
      send_id(app, sel(c"setMainMenu:"), bar);
      send(bar, sel(c"release"));
    }
    CHOICES.with_borrow_mut(|c| *c = (ids, Some(on_choose)));
  }
}

/// The system's open and save dialogs (M11, D0017): `NSOpenPanel` and
/// `NSSavePanel`, run as modal dialogs.
pub struct MacFileDialogs;

/// `NSModalResponseOK`.
const MODAL_OK: isize = 1;

impl dovetail_core::FileDialogs for MacFileDialogs {
  fn show(
    &self,
    dialog: dovetail_core::FileDialog,
    done: Callback<Option<std::path::PathBuf>>,
  ) -> bool {
    let open = dialog.kind == dovetail_core::FileDialogKind::Open;
    let panel_class = class(if open { c"NSOpenPanel" } else { c"NSSavePanel" });
    if panel_class.is_null() {
      return false;
    }
    let _pool = Pool::new();
    let mut owned = Vec::new();
    let mut string = |text: &str| {
      let s = ns_string(text);
      owned.push(s);
      s
    };
    let title = string(&dialog.title);
    let folder = dialog
      .folder
      .as_ref()
      .and_then(|f| f.to_str())
      .map(&mut string);
    let name = dialog.name.as_deref().map(&mut string);
    let types: Vec<Id> = dialog
      .filters
      .iter()
      .flat_map(|f| f.extensions.iter())
      .map(|e| string(e))
      .collect();
    // Any file is allowed when a filter takes any file.
    let any = dialog.filters.is_empty()
      || dialog
        .filters
        .iter()
        .any(|f| f.extensions.is_empty());
    // SAFETY: each objc_msgSend cast matches its method's signature:
    // `openPanel`/`savePanel` and `runModal`, `URL` and `path` take
    // nothing; `setTitle:`, `setMessage:`, `setNameFieldStringValue:`,
    // `setDirectoryURL:` and `setAllowedFileTypes:` take one object;
    // `fileURLWithPath:` takes an NSString; `arrayWithObjects:count:`
    // takes a pointer to `types` (read during the call) and its length;
    // the `setCan...:` and `setAllows...:` setters take a BOOL;
    // `UTF8String` returns a C string that lives as long as the pool,
    // which outlives the copy made here. The strings made by `ns_string`
    // are owned here and released once, after the panel is done.
    let chosen = unsafe {
      let send: unsafe extern "C" fn(Id, Sel) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let send_id: unsafe extern "C" fn(Id, Sel, Id) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let send_bool: unsafe extern "C" fn(Id, Sel, i8) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let send_run: unsafe extern "C" fn(Id, Sel) -> isize =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let send_array: unsafe extern "C" fn(Id, Sel, *const Id, usize) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      let send_str: unsafe extern "C" fn(Id, Sel) -> *const c_char =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());

      let panel = send(
        panel_class,
        sel(if open { c"openPanel" } else { c"savePanel" }),
      );
      if panel.is_null() {
        None
      } else {
        send_id(panel, sel(c"setTitle:"), title);
        send_id(panel, sel(c"setMessage:"), title);
        if open {
          send_bool(panel, sel(c"setCanChooseFiles:"), 1);
          send_bool(panel, sel(c"setCanChooseDirectories:"), 0);
          send_bool(panel, sel(c"setAllowsMultipleSelection:"), 0);
        }
        if let Some(folder) = folder {
          let url = send_id(class(c"NSURL"), sel(c"fileURLWithPath:"), folder);
          send_id(panel, sel(c"setDirectoryURL:"), url);
        }
        if let Some(name) = name {
          send_id(panel, sel(c"setNameFieldStringValue:"), name);
        }
        if !any && !types.is_empty() {
          let array = send_array(
            class(c"NSArray"),
            sel(c"arrayWithObjects:count:"),
            types.as_ptr(),
            types.len(),
          );
          send_id(panel, sel(c"setAllowedFileTypes:"), array);
        }
        if send_run(panel, sel(c"runModal")) == MODAL_OK {
          let url = send(panel, sel(c"URL"));
          let path = if url.is_null() {
            std::ptr::null_mut()
          } else {
            send(url, sel(c"path"))
          };
          let utf8 = if path.is_null() {
            std::ptr::null()
          } else {
            send_str(path, sel(c"UTF8String"))
          };
          (!utf8.is_null()).then(|| {
            std::path::PathBuf::from(
              CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned(),
            )
          })
        } else {
          None
        }
      }
    };
    for s in owned {
      // SAFETY: each string was made by `ns_string`, is owned here, and
      // is released once.
      unsafe {
        let send: unsafe extern "C" fn(Id, Sel) =
          std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        send(s, sel(c"release"));
      }
    }
    done.call(chosen);
    true
  }
}

/// True when the system shows scroll bars only while scrolling: the
/// overlay style that "Show scroll bars" gives on most Macs (M15).
pub fn overlay_scrollbars() -> bool {
  let _pool = Pool::new();
  // SAFETY: the cast matches `+[NSScroller preferredScrollerStyle]`, which
  // takes nothing and returns an NSInteger (isize). A missing class gives
  // nil, which is checked.
  unsafe {
    let send: unsafe extern "C" fn(Id, Sel) -> isize =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let scroller = class(c"NSScroller");
    // NSScrollerStyleOverlay is 1.
    !scroller.is_null() && send(scroller, sel(c"preferredScrollerStyle")) == 1
  }
}

/// Light or dark from the user's interface style, the accent colour, and
/// whether the user asked for more contrast (M14).
pub fn appearance() -> dovetail_core::SystemAppearance {
  let _pool = Pool::new();
  let key = ns_string("AppleInterfaceStyle");
  // SAFETY: each objc_msgSend cast matches its method's signature:
  // `standardUserDefaults`, `controlAccentColor`, `sRGBColorSpace` and
  // `sharedWorkspace` take nothing and return objects (or nil);
  // `stringForKey:` and `colorUsingColorSpace:` take one object;
  // `UTF8String` returns a C string that lives as long as the pool, which
  // outlives the copy made here; `redComponent`, `greenComponent` and
  // `blueComponent` return a CGFloat (f64);
  // `accessibilityDisplayShouldIncreaseContrast` returns a BOOL. Classes
  // that are missing on older systems give nil, which is checked. `key`
  // was made by `ns_string`, is owned here and released once.
  unsafe {
    let send: unsafe extern "C" fn(Id, Sel) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let send_id: unsafe extern "C" fn(Id, Sel, Id) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let send_str: unsafe extern "C" fn(Id, Sel) -> *const c_char =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let send_f64: unsafe extern "C" fn(Id, Sel) -> f64 =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let send_bool: unsafe extern "C" fn(Id, Sel) -> i8 =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let release: unsafe extern "C" fn(Id, Sel) =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());

    let defaults = send(class(c"NSUserDefaults"), sel(c"standardUserDefaults"));
    let style = if defaults.is_null() {
      std::ptr::null_mut()
    } else {
      send_id(defaults, sel(c"stringForKey:"), key)
    };
    let dark = !style.is_null() && {
      let text = send_str(style, sel(c"UTF8String"));
      !text.is_null() && CStr::from_ptr(text).to_bytes() == b"Dark"
    };
    release(key, sel(c"release"));

    let colour_class = class(c"NSColor");
    let space_class = class(c"NSColorSpace");
    let accent = if colour_class.is_null() || space_class.is_null() {
      None
    } else {
      let colour = send(colour_class, sel(c"controlAccentColor"));
      let srgb = send(space_class, sel(c"sRGBColorSpace"));
      let colour = if colour.is_null() || srgb.is_null() {
        std::ptr::null_mut()
      } else {
        send_id(colour, sel(c"colorUsingColorSpace:"), srgb)
      };
      (!colour.is_null()).then(|| {
        dovetail_core::Color::rgb(
          send_f64(colour, sel(c"redComponent")) as f32,
          send_f64(colour, sel(c"greenComponent")) as f32,
          send_f64(colour, sel(c"blueComponent")) as f32,
        )
      })
    };

    let workspace_class = class(c"NSWorkspace");
    let workspace = if workspace_class.is_null() {
      std::ptr::null_mut()
    } else {
      send(workspace_class, sel(c"sharedWorkspace"))
    };
    let high_contrast = !workspace.is_null()
      && send_bool(
        workspace,
        sel(c"accessibilityDisplayShouldIncreaseContrast"),
      ) != 0;

    dovetail_core::SystemAppearance { dark, accent, high_contrast }
  }
}

/// Less motion, the caret blink, the double-click time and the scroll bar
/// style, as set in macOS (M18). macOS has no text scale for applications.
pub fn settings() -> dovetail_core::SystemSettings {
  let overlay = overlay_scrollbars();
  let _pool = Pool::new();
  let key = ns_string("NSTextInsertionPointBlinkPeriodOn");
  // SAFETY: each objc_msgSend cast matches its method's signature:
  // `sharedWorkspace` and `standardUserDefaults` take nothing and return
  // objects (or nil); `accessibilityDisplayShouldReduceMotion` returns a
  // BOOL; `+[NSEvent doubleClickInterval]` returns an NSTimeInterval
  // (f64); `integerForKey:` takes one object and returns an NSInteger
  // (isize), 0 when the key is not set. Missing classes give nil, which
  // is checked. `key` was made by `ns_string`, is owned here and released
  // once.
  unsafe {
    let send: unsafe extern "C" fn(Id, Sel) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let send_bool: unsafe extern "C" fn(Id, Sel) -> i8 =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let send_f64: unsafe extern "C" fn(Id, Sel) -> f64 =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let send_int: unsafe extern "C" fn(Id, Sel, Id) -> isize =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let release: unsafe extern "C" fn(Id, Sel) =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());

    let workspace_class = class(c"NSWorkspace");
    let workspace = if workspace_class.is_null() {
      std::ptr::null_mut()
    } else {
      send(workspace_class, sel(c"sharedWorkspace"))
    };
    let reduce_motion = !workspace.is_null()
      && send_bool(workspace, sel(c"accessibilityDisplayShouldReduceMotion"))
        != 0;

    let event_class = class(c"NSEvent");
    let double_click_seconds = if event_class.is_null() {
      f64::NAN
    } else {
      send_f64(event_class, sel(c"doubleClickInterval"))
    };

    let defaults = send(class(c"NSUserDefaults"), sel(c"standardUserDefaults"));
    let blink_on = if defaults.is_null() {
      0
    } else {
      send_int(defaults, sel(c"integerForKey:"), key)
    };
    release(key, sel(c"release"));

    crate::system_settings::from_macos(&crate::system_settings::MacValues {
      reduce_motion,
      double_click_seconds,
      blink_on_ms: (blink_on > 0).then_some(blink_on as i64),
      overlay_scrollbars: overlay,
    })
  }
}
