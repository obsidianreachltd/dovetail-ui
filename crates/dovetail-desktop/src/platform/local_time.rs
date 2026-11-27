//! Today's date in the user's time zone on Linux and macOS, through the C
//! library's `time` and `localtime_r` (M25, D0030).
#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, c_long};

/// `time_t`: seconds since 1970, a `long` on the systems Dovetail runs on.
type TimeT = c_long;

/// `struct tm`, as glibc, musl and macOS lay it out: nine `int`s, then the
/// offset from UTC and the zone's name.
#[repr(C)]
struct Tm {
  second: c_int,
  minute: c_int,
  hour: c_int,
  day: c_int,
  month: c_int,
  year: c_int,
  weekday: c_int,
  yearday: c_int,
  dst: c_int,
  gmtoff: c_long,
  zone: *const c_char,
}

unsafe extern "C" {
  fn time(out: *mut TimeT) -> TimeT;
  fn localtime_r(
    time: *const TimeT,
    out: *mut Tm,
  ) -> *mut Tm;
}

/// Today's local date, or `None` if the C library cannot tell.
pub fn local_date() -> Option<dovetail_core::Date> {
  let mut tm = Tm {
    second: 0,
    minute: 0,
    hour: 0,
    day: 0,
    month: 0,
    year: 0,
    weekday: 0,
    yearday: 0,
    dst: 0,
    gmtoff: 0,
    zone: std::ptr::null(),
  };
  // SAFETY: `time` with a null pointer only returns the time. `localtime_r`
  // reads one `time_t` and writes one `struct tm`, both pointing at live
  // values of those layouts; it returns null when it fails.
  let filled = unsafe {
    let now = time(std::ptr::null_mut());
    !localtime_r(&now, &mut tm).is_null()
  };
  if !filled {
    return None;
  }
  dovetail_core::Date::new(
    tm.year + 1900,
    u32::try_from(tm.month + 1).ok()?,
    u32::try_from(tm.day).ok()?,
  )
}
