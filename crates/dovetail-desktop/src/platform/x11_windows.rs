//! X11 window management that winit does not do (M19, D0029), through
//! `libX11`, loaded when first needed with `dlopen` (D0013):
//!
//! - the work area, from the window manager's `_NET_WORKAREA` on the root
//!   window, for the current desktop;
//! - a modal window's owner, with `WM_TRANSIENT_FOR`, so the window manager
//!   keeps it in front of its owner.
//!
//! Both use one connection of their own, opened on the UI thread and kept.
//! Window properties live in the X server, so another connection can set
//! them on winit's windows.
#![allow(unsafe_code)]

use std::cell::OnceCell;
use std::ffi::{CStr, c_char, c_int, c_long, c_uchar, c_ulong, c_void};

use dovetail_core::ScreenRect;

type Display = c_void;
type Window = c_ulong;
type Atom = c_ulong;

const RTLD_NOW: c_int = 2;
const XA_CARDINAL: Atom = 6;
const SUCCESS: c_int = 0;

#[link(name = "dl")]
unsafe extern "C" {
  fn dlopen(
    file: *const c_char,
    mode: c_int,
  ) -> *mut c_void;
  fn dlsym(
    handle: *mut c_void,
    name: *const c_char,
  ) -> *mut c_void;
}

struct Xlib {
  display: *mut Display,
  root: Window,
  intern_atom: unsafe extern "C" fn(*mut Display, *const c_char, c_int) -> Atom,
  get_window_property: unsafe extern "C" fn(
    *mut Display,
    Window,
    Atom,
    c_long,
    c_long,
    c_int,
    Atom,
    *mut Atom,
    *mut c_int,
    *mut c_ulong,
    *mut c_ulong,
    *mut *mut c_uchar,
  ) -> c_int,
  free: unsafe extern "C" fn(*mut c_void) -> c_int,
  flush: unsafe extern "C" fn(*mut Display) -> c_int,
  set_transient_for_hint:
    unsafe extern "C" fn(*mut Display, Window, Window) -> c_int,
}

/// # Safety
///
/// `p` must be a function whose C signature is `T`.
unsafe fn cast<T: Copy>(p: *mut c_void) -> T {
  // SAFETY: the caller promises `p` has the signature `T`, which is a
  // function pointer the size of `p`.
  unsafe { std::mem::transmute_copy(&p) }
}

impl Xlib {
  fn open() -> Option<Xlib> {
    // SAFETY: dlopen takes a C string and returns null on failure. The
    // library is never closed, so the symbols stay valid for the process.
    let lib = unsafe { dlopen(c"libX11.so.6".as_ptr(), RTLD_NOW) };
    if lib.is_null() {
      return None;
    }
    let symbol = |name: &CStr| {
      // SAFETY: `lib` is a live handle and `name` a C string.
      let p = unsafe { dlsym(lib, name.as_ptr()) };
      (!p.is_null()).then_some(p)
    };
    // SAFETY: each symbol is the libX11 function of that name, cast to its
    // C signature from Xlib.h. XOpenDisplay with null opens $DISPLAY, or
    // gives null when there is none; the display is kept for the process.
    unsafe {
      let open_display: unsafe extern "C" fn(*const c_char) -> *mut Display =
        cast(symbol(c"XOpenDisplay")?);
      let default_root: unsafe extern "C" fn(*mut Display) -> Window =
        cast(symbol(c"XDefaultRootWindow")?);
      let x = Xlib {
        display: std::ptr::null_mut(),
        root: 0,
        intern_atom: cast(symbol(c"XInternAtom")?),
        get_window_property: cast(symbol(c"XGetWindowProperty")?),
        free: cast(symbol(c"XFree")?),
        flush: cast(symbol(c"XFlush")?),
        set_transient_for_hint: cast(symbol(c"XSetTransientForHint")?),
      };
      let display = open_display(std::ptr::null());
      if display.is_null() {
        return None;
      }
      Some(Xlib {
        display,
        root: default_root(display),
        ..x
      })
    }
  }

  fn atom(
    &self,
    name: &CStr,
  ) -> Atom {
    // SAFETY: the display is live, and `name` a C string. 0 asks for the
    // atom to be made if it is not there yet.
    unsafe { (self.intern_atom)(self.display, name.as_ptr(), 0) }
  }

  /// The 32-bit numbers of a `CARDINAL` property of the root window.
  fn root_cardinals(
    &self,
    name: &CStr,
  ) -> Option<Vec<i64>> {
    let property = self.atom(name);
    let mut kind: Atom = 0;
    let mut format: c_int = 0;
    let mut count: c_ulong = 0;
    let mut after: c_ulong = 0;
    let mut data: *mut c_uchar = std::ptr::null_mut();
    // SAFETY: the display and root window are live; every out pointer
    // points at a local of the right type. On success Xlib gives `count`
    // items of `format` bits, which for 32 are stored as C longs, and the
    // data is freed with XFree below.
    unsafe {
      let status = (self.get_window_property)(
        self.display,
        self.root,
        property,
        0,
        1024,
        0,
        XA_CARDINAL,
        &mut kind,
        &mut format,
        &mut count,
        &mut after,
        &mut data,
      );
      if status != SUCCESS || data.is_null() {
        return None;
      }
      let values = if format == 32 && kind == XA_CARDINAL {
        std::slice::from_raw_parts(data.cast::<c_long>(), count as usize)
          .iter()
          // A C long is 64 bits here, but not on every system.
          .map(|v| {
            #[allow(clippy::useless_conversion)]
            i64::from(*v)
          })
          .collect()
      } else {
        Vec::new()
      };
      (self.free)(data.cast());
      Some(values)
    }
  }
}

thread_local! {
  static XLIB: OnceCell<Option<Xlib>> = const { OnceCell::new() };
}

fn with_xlib<R>(work: impl FnOnce(&Xlib) -> R) -> Option<R> {
  XLIB.with(|x| x.get_or_init(Xlib::open).as_ref().map(work))
}

/// The work area of the current desktop, from the window manager, in
/// physical pixels, cut down to `bounds`. `None` when the window manager
/// does not say, as with none at all.
pub(crate) fn work_area(bounds: ScreenRect) -> Option<ScreenRect> {
  with_xlib(|x| {
    let areas = x.root_cardinals(c"_NET_WORKAREA")?;
    let desktop = x
      .root_cardinals(c"_NET_CURRENT_DESKTOP")
      .and_then(|d| d.first().copied())
      .unwrap_or(0);
    let start = usize::try_from(desktop).ok()?.checked_mul(4)?;
    let a = areas.get(start..start + 4)?;
    let area =
      ScreenRect::new(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32);
    // `_NET_WORKAREA` covers every monitor at once; each monitor gets the
    // part of it on that monitor.
    let x0 = bounds.x.max(area.x);
    let y0 = bounds.y.max(area.y);
    let x1 = (bounds.x + bounds.width).min(area.x + area.width);
    let y1 = (bounds.y + bounds.height).min(area.y + area.height);
    (x1 > x0 && y1 > y0).then(|| ScreenRect::new(x0, y0, x1 - x0, y1 - y0))
  })
  .flatten()
}

/// Tells the window manager that `child` belongs to `owner`, which keeps it
/// in front of its owner. Both are X11 window ids.
pub(crate) fn set_transient_for(
  child: c_ulong,
  owner: c_ulong,
) {
  with_xlib(|x| {
    // SAFETY: the display is live, and both ids are windows winit made and
    // still has open: the caller holds them. A bad id would make Xlib's
    // default error handler end the program.
    unsafe {
      (x.set_transient_for_hint)(x.display, child, owner);
      (x.flush)(x.display);
    }
  });
}
