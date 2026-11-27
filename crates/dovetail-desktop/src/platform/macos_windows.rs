//! macOS window management that winit does not do (M19, D0029), through
//! the Objective-C runtime in `libobjc` (D0013):
//!
//! - the work area: an `NSScreen`'s `visibleFrame`, without the menu bar
//!   and the Dock;
//! - a modal window's owner: the owner's `NSWindow` takes the modal one as
//!   a child window, so it stays in front of its owner and moves with it;
//! - a click on the Dock icon with no window open: a method added to the
//!   application's delegate, `applicationShouldHandleReopen:hasVisibleWindows:`;
//! - whether a window really is full screen, from its style mask, and
//!   leaving full screen when AppKit refused an earlier ask that winit
//!   counted as done.
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};

type Id = *mut c_void;
type Sel = *const c_void;

/// `NSWindowAbove`.
const ABOVE: isize = 1;

/// `NSWindowStyleMaskFullScreen`.
const FULL_SCREEN_MASK: usize = 1 << 14;

#[link(name = "objc")]
unsafe extern "C" {
  fn objc_getClass(name: *const c_char) -> Id;
  fn sel_registerName(name: *const c_char) -> Sel;
  fn objc_msgSend();
  #[cfg(target_arch = "x86_64")]
  fn objc_msgSend_stret();
  fn object_getClass(object: Id) -> Id;
  fn class_addMethod(
    class: Id,
    name: Sel,
    imp: *const c_void,
    types: *const c_char,
  ) -> i8;
}

/// `NSRect`: an origin and a size, in points.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NsRect {
  x: f64,
  y: f64,
  width: f64,
  height: f64,
}

fn sel(name: &CStr) -> Sel {
  // SAFETY: the name is a valid C string; selectors live for the process.
  unsafe { sel_registerName(name.as_ptr()) }
}

/// Sends a message that takes nothing and returns an object.
///
/// # Safety
///
/// `to` must be an object, or null, that answers `name` with an object.
unsafe fn send_id(
  to: Id,
  name: &CStr,
) -> Id {
  if to.is_null() {
    return std::ptr::null_mut();
  }
  // SAFETY: the caller promises the method takes no arguments and returns
  // an object; objc_msgSend is cast to exactly that signature.
  unsafe {
    let send: unsafe extern "C" fn(Id, Sel) -> Id =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    send(to, sel(name))
  }
}

/// Sends a message that takes nothing and returns an `NSRect`.
///
/// # Safety
///
/// `to` must be a live object that answers `name` with an `NSRect`.
unsafe fn send_rect(
  to: Id,
  name: &CStr,
) -> NsRect {
  // SAFETY: the caller promises the method returns an NSRect and takes
  // nothing. On x86_64 a struct that big is returned through memory, with
  // objc_msgSend_stret, which takes the place to write it first; on arm64
  // it comes back in registers from objc_msgSend.
  unsafe {
    #[cfg(target_arch = "x86_64")]
    {
      let send: unsafe extern "C" fn(*mut NsRect, Id, Sel) =
        std::mem::transmute(objc_msgSend_stret as unsafe extern "C" fn());
      let mut rect = NsRect::default();
      send(&mut rect, to, sel(name));
      rect
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
      let send: unsafe extern "C" fn(Id, Sel) -> NsRect =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
      send(to, sel(name))
    }
  }
}

/// How far an `NSScreen`'s visible frame is inside its frame, in points:
/// left, top, right and bottom. `screen` is winit's `ns_screen` pointer.
pub(crate) fn work_area_insets(screen: *mut c_void) -> Option<[f64; 4]> {
  if screen.is_null() {
    return None;
  }
  // SAFETY: winit gives a live NSScreen, which answers both with NSRects.
  let (frame, visible) = unsafe {
    (
      send_rect(screen, c"frame"),
      send_rect(screen, c"visibleFrame"),
    )
  };
  if visible.width <= 0.0 || visible.height <= 0.0 {
    return None;
  }
  // AppKit's y goes up from the bottom, so the top inset is measured from
  // the frame's top edge.
  Some([
    visible.x - frame.x,
    (frame.y + frame.height) - (visible.y + visible.height),
    (frame.x + frame.width) - (visible.x + visible.width),
    visible.y - frame.y,
  ])
}

/// Makes the window of the `NSView` `child` a child window of the window
/// of the `NSView` `owner`, above it. Both are winit's views, which live
/// as long as their windows.
pub(crate) fn add_child_window(
  owner: *mut c_void,
  child: *mut c_void,
) {
  // SAFETY: both views are live NSViews; `-[NSView window]` returns their
  // NSWindow, or null. `-[NSWindow addChildWindow:ordered:]` takes a window
  // and an NSWindowOrderingMode, an NSInteger; objc_msgSend is cast to
  // exactly that signature.
  unsafe {
    let owner = send_id(owner, c"window");
    let child = send_id(child, c"window");
    if owner.is_null() || child.is_null() {
      return;
    }
    let send: unsafe extern "C" fn(Id, Sel, Id, isize) =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    send(owner, sel(c"addChildWindow:ordered:"), child, ABOVE);
  }
}

/// Whether the window of the `NSView` `view` is full screen, as AppKit
/// says. `view` is winit's view, which lives as long as its window.
pub(crate) fn is_full_screen(view: *mut c_void) -> bool {
  // SAFETY: the view is a live NSView; `-[NSView window]` returns its
  // NSWindow, or null. `-[NSWindow styleMask]` takes nothing and returns an
  // NSWindowStyleMask, an NSUInteger; objc_msgSend is cast to exactly that
  // signature.
  unsafe {
    let window = send_id(view, c"window");
    if window.is_null() {
      return false;
    }
    let send: unsafe extern "C" fn(Id, Sel) -> usize =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    send(window, sel(c"styleMask")) & FULL_SCREEN_MASK != 0
  }
}

/// Toggles full screen for the window of the `NSView` `view`. AppKit
/// refuses it while the window is changing to or from full screen.
pub(crate) fn toggle_full_screen(view: *mut c_void) {
  // SAFETY: the view is a live NSView; `-[NSView window]` returns its
  // NSWindow, or null. `-[NSWindow toggleFullScreen:]` takes an object,
  // the sender, which may be nil; objc_msgSend is cast to exactly that
  // signature.
  unsafe {
    let window = send_id(view, c"window");
    if window.is_null() {
      return;
    }
    let send: unsafe extern "C" fn(Id, Sel, Id) =
      std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    send(window, sel(c"toggleFullScreen:"), std::ptr::null_mut());
  }
}

thread_local! {
  static REOPEN: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// `-applicationShouldHandleReopen:hasVisibleWindows:`, added to the
/// application's delegate.
extern "C" fn should_reopen(
  _: Id,
  _: Sel,
  _: Id,
  visible: i8,
) -> i8 {
  if visible == 0 {
    REOPEN.with_borrow(|r| {
      if let Some(reopen) = r {
        reopen();
      }
    });
  }
  1
}

/// Calls `reopen` when the user clicks the Dock icon while no window is
/// open. Call it once the event loop runs, when the application has its
/// delegate.
pub(crate) fn on_reopen(reopen: Box<dyn Fn()>) {
  REOPEN.with_borrow_mut(|r| *r = Some(reopen));
  // SAFETY: `+[NSApplication sharedApplication]` and `-delegate` take
  // nothing and return objects (the delegate may be null). The method is
  // added with the type string of a method that takes an object and a
  // BOOL and returns a BOOL, which `should_reopen` matches. If the class
  // has the method already, class_addMethod does nothing.
  unsafe {
    let app_class = objc_getClass(c"NSApplication".as_ptr());
    let app = send_id(app_class, c"sharedApplication");
    let delegate = send_id(app, c"delegate");
    if delegate.is_null() {
      return;
    }
    let class = object_getClass(delegate);
    class_addMethod(
      class,
      sel(c"applicationShouldHandleReopen:hasVisibleWindows:"),
      should_reopen as *const c_void,
      c"c@:@c".as_ptr(),
    );
  }
}
