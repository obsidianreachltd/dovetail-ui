//! The X11 clipboard (the `CLIPBOARD` selection), through `libX11`, which
//! is loaded when first needed with `dlopen` (D0013). Most Wayland desktops
//! run XWayland, which passes the clipboard between X11 and Wayland.
//!
//! On X11 the program that copied text keeps it and sends it to each
//! program that pastes. So a thread with its own X connection and a hidden
//! window owns the selection and answers those requests. The UI thread
//! sends it commands over a channel and wakes it through a pipe.
//!
//! Not yet handled: the `INCR` protocol for very large text, and handing
//! the text to a clipboard manager when the application exits.
#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use dovetail_core::Clipboard;

type Display = c_void;
type Window = c_ulong;
type Atom = c_ulong;

const SELECTION_CLEAR: c_int = 29;
const SELECTION_REQUEST: c_int = 30;
const SELECTION_NOTIFY: c_int = 31;
const XA_ATOM: Atom = 4;
const PROP_MODE_REPLACE: c_int = 0;
const CURRENT_TIME: c_ulong = 0;
const RTLD_NOW: c_int = 2;
const POLLIN: i16 = 1;

/// How long a paste waits for the program that owns the clipboard.
const READ_TIMEOUT: Duration = Duration::from_secs(1);

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

unsafe extern "C" {
  fn pipe(fds: *mut c_int) -> c_int;
  fn read(
    fd: c_int,
    buf: *mut c_void,
    count: usize,
  ) -> isize;
  fn write(
    fd: c_int,
    buf: *const c_void,
    count: usize,
  ) -> isize;
  fn poll(
    fds: *mut PollFd,
    count: c_ulong,
    timeout: c_int,
  ) -> c_int;
}

#[repr(C)]
struct PollFd {
  fd: c_int,
  events: i16,
  revents: i16,
}

/// `XEvent`: a union 24 longs long. Its first field is the event type.
#[repr(C)]
#[derive(Clone, Copy)]
struct XEvent {
  data: [c_long; 24],
}

impl XEvent {
  fn kind(&self) -> c_int {
    // The type is the first `int` of every event.
    self.data[0] as c_int
  }
}

/// `XSelectionRequestEvent`.
#[repr(C)]
#[derive(Clone, Copy)]
struct SelectionRequest {
  kind: c_int,
  serial: c_ulong,
  send_event: c_int,
  display: *mut Display,
  owner: Window,
  requestor: Window,
  selection: Atom,
  target: Atom,
  property: Atom,
  time: c_ulong,
}

/// `XSelectionEvent`, sent as `SelectionNotify`.
#[repr(C)]
#[derive(Clone, Copy)]
struct SelectionNotify {
  kind: c_int,
  serial: c_ulong,
  send_event: c_int,
  display: *mut Display,
  requestor: Window,
  selection: Atom,
  target: Atom,
  property: Atom,
  time: c_ulong,
}

/// The `libX11` functions used, looked up by name.
struct Xlib {
  open_display: unsafe extern "C" fn(*const c_char) -> *mut Display,
  default_root_window: unsafe extern "C" fn(*mut Display) -> Window,
  create_simple_window: unsafe extern "C" fn(
    *mut Display,
    Window,
    c_int,
    c_int,
    c_uint,
    c_uint,
    c_uint,
    c_ulong,
    c_ulong,
  ) -> Window,
  intern_atom: unsafe extern "C" fn(*mut Display, *const c_char, c_int) -> Atom,
  set_selection_owner:
    unsafe extern "C" fn(*mut Display, Atom, Window, c_ulong) -> c_int,
  get_selection_owner: unsafe extern "C" fn(*mut Display, Atom) -> Window,
  convert_selection: unsafe extern "C" fn(
    *mut Display,
    Atom,
    Atom,
    Atom,
    Window,
    c_ulong,
  ) -> c_int,
  pending: unsafe extern "C" fn(*mut Display) -> c_int,
  next_event: unsafe extern "C" fn(*mut Display, *mut XEvent) -> c_int,
  send_event: unsafe extern "C" fn(
    *mut Display,
    Window,
    c_int,
    c_long,
    *mut XEvent,
  ) -> c_int,
  change_property: unsafe extern "C" fn(
    *mut Display,
    Window,
    Atom,
    Atom,
    c_int,
    c_int,
    *const u8,
    c_int,
  ) -> c_int,
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
    *mut *mut u8,
  ) -> c_int,
  free: unsafe extern "C" fn(*mut c_void) -> c_int,
  flush: unsafe extern "C" fn(*mut Display) -> c_int,
  connection_number: unsafe extern "C" fn(*mut Display) -> c_int,
}

/// A looked-up symbol as the function pointer type `F`.
///
/// # Safety
///
/// `F` must be the function pointer type of the symbol's C signature.
unsafe fn cast<F: Copy>(symbol: *mut c_void) -> F {
  assert_eq!(size_of::<F>(), size_of::<*mut c_void>());
  // SAFETY: the sizes match, and the caller promises the type is right.
  unsafe { std::mem::transmute_copy(&symbol) }
}

impl Xlib {
  fn load() -> Option<Xlib> {
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
    // SAFETY: each symbol is the libX11 function of that name, and is cast
    // to its C signature from Xlib.h.
    unsafe {
      Some(Xlib {
        open_display: cast(symbol(c"XOpenDisplay")?),
        default_root_window: cast(symbol(c"XDefaultRootWindow")?),
        create_simple_window: cast(symbol(c"XCreateSimpleWindow")?),
        intern_atom: cast(symbol(c"XInternAtom")?),
        set_selection_owner: cast(symbol(c"XSetSelectionOwner")?),
        get_selection_owner: cast(symbol(c"XGetSelectionOwner")?),
        convert_selection: cast(symbol(c"XConvertSelection")?),
        pending: cast(symbol(c"XPending")?),
        next_event: cast(symbol(c"XNextEvent")?),
        send_event: cast(symbol(c"XSendEvent")?),
        change_property: cast(symbol(c"XChangeProperty")?),
        get_window_property: cast(symbol(c"XGetWindowProperty")?),
        free: cast(symbol(c"XFree")?),
        flush: cast(symbol(c"XFlush")?),
        connection_number: cast(symbol(c"XConnectionNumber")?),
      })
    }
  }
}

enum Command {
  Own(String),
  Read(Sender<Option<String>>),
}

/// The selection owner: its own connection and hidden window, used only on
/// its own thread.
struct Owner {
  x: Xlib,
  display: *mut Display,
  window: Window,
  clipboard: Atom,
  targets: Atom,
  utf8: Atom,
  text_atoms: [Atom; 4],
  incr: Atom,
  property: Atom,
  /// The text this program owns the clipboard with.
  owned: Option<String>,
}

impl Owner {
  fn open() -> Option<Owner> {
    let x = Xlib::load()?;
    // SAFETY: null asks for the display named by $DISPLAY; null back means
    // there is none. The other calls take that live display and C strings,
    // and create one 1×1 window that is never mapped.
    unsafe {
      let display = (x.open_display)(std::ptr::null());
      if display.is_null() {
        return None;
      }
      let root = (x.default_root_window)(display);
      let window = (x.create_simple_window)(display, root, 0, 0, 1, 1, 0, 0, 0);
      let atom = |name: &CStr| (x.intern_atom)(display, name.as_ptr(), 0);
      let utf8 = atom(c"UTF8_STRING");
      let text_atoms = [
        utf8,
        atom(c"text/plain;charset=utf-8"),
        atom(c"STRING"),
        atom(c"TEXT"),
      ];
      Some(Owner {
        clipboard: atom(c"CLIPBOARD"),
        targets: atom(c"TARGETS"),
        incr: atom(c"INCR"),
        property: atom(c"DOVETAIL_CLIPBOARD"),
        utf8,
        text_atoms,
        x,
        display,
        window,
        owned: None,
      })
    }
  }

  /// Runs until the UI side is dropped.
  fn run(
    mut self,
    commands: Receiver<Command>,
    wake: c_int,
  ) {
    // SAFETY: the display is live for the life of `self`.
    let x_fd = unsafe { (self.x.connection_number)(self.display) };
    loop {
      self.handle_events(false);
      loop {
        match commands.try_recv() {
          Ok(Command::Own(text)) => self.own(text),
          Ok(Command::Read(reply)) => {
            let text = self.read();
            let _ = reply.send(text);
          },
          Err(TryRecvError::Empty) => break,
          Err(TryRecvError::Disconnected) => return,
        }
      }
      self.handle_events(false);
      let mut fds = [
        PollFd { fd: x_fd, events: POLLIN, revents: 0 },
        PollFd { fd: wake, events: POLLIN, revents: 0 },
      ];
      // SAFETY: `fds` is a live array of two pollfd records.
      unsafe { poll(fds.as_mut_ptr(), 2, -1) };
      if fds[1].revents != 0 {
        let mut buf = [0u8; 64];
        // SAFETY: reads at most 64 bytes into `buf` from our own pipe.
        unsafe { read(wake, buf.as_mut_ptr().cast(), buf.len()) };
      }
    }
  }

  /// Handles every event already waiting. With `want_notify`, returns the
  /// first `SelectionNotify` for our window instead of ignoring it.
  fn handle_events(
    &mut self,
    want_notify: bool,
  ) -> Option<SelectionNotify> {
    let mut found = None;
    // SAFETY: the display is live; XNextEvent fills a whole XEvent, and
    // each event is read as the struct for its type.
    unsafe {
      while (self.x.pending)(self.display) > 0 {
        let mut event = XEvent { data: [0; 24] };
        (self.x.next_event)(self.display, &mut event);
        match event.kind() {
          SELECTION_REQUEST => {
            let request = *(&event as *const XEvent).cast::<SelectionRequest>();
            self.answer(request);
          },
          SELECTION_CLEAR => self.owned = None,
          SELECTION_NOTIFY if want_notify && found.is_none() => {
            let notify = *(&event as *const XEvent).cast::<SelectionNotify>();
            if notify.requestor == self.window {
              found = Some(notify);
            }
          },
          _ => {},
        }
      }
    }
    found
  }

  fn own(
    &mut self,
    text: String,
  ) {
    self.owned = Some(text);
    // SAFETY: the display and window are live.
    unsafe {
      (self.x.set_selection_owner)(
        self.display,
        self.clipboard,
        self.window,
        CURRENT_TIME,
      );
      (self.x.flush)(self.display);
    }
  }

  /// Sends another program the text it asked for.
  fn answer(
    &self,
    request: SelectionRequest,
  ) {
    // Old clients leave the property empty and mean the target.
    let property = if request.property == 0 {
      request.target
    } else {
      request.property
    };
    let mut done = false;
    if let Some(text) = &self.owned
      && request.selection == self.clipboard
    {
      // SAFETY: the display is live and the data pointers and lengths
      // describe live buffers: `targets` as longs (format 32), `text` as
      // bytes (format 8).
      unsafe {
        if request.target == self.targets {
          let mut targets: Vec<c_ulong> = vec![self.targets];
          targets.extend(self.text_atoms);
          (self.x.change_property)(
            self.display,
            request.requestor,
            property,
            XA_ATOM,
            32,
            PROP_MODE_REPLACE,
            targets.as_ptr().cast(),
            targets.len() as c_int,
          );
          done = true;
        } else if self.text_atoms.contains(&request.target) {
          (self.x.change_property)(
            self.display,
            request.requestor,
            property,
            self.utf8,
            8,
            PROP_MODE_REPLACE,
            text.as_ptr(),
            text.len() as c_int,
          );
          done = true;
        }
      }
    }
    let notify = SelectionNotify {
      kind: SELECTION_NOTIFY,
      serial: 0,
      send_event: 1,
      display: self.display,
      requestor: request.requestor,
      selection: request.selection,
      target: request.target,
      property: if done { property } else { 0 },
      time: request.time,
    };
    let mut event = XEvent { data: [0; 24] };
    // SAFETY: SelectionNotify is smaller than XEvent, so it fits; the
    // display is live and the event is sent to the requesting window.
    unsafe {
      *(&mut event as *mut XEvent).cast::<SelectionNotify>() = notify;
      (self.x.send_event)(self.display, request.requestor, 0, 0, &mut event);
      (self.x.flush)(self.display);
    }
  }

  /// The clipboard's text, asking the program that owns it.
  fn read(&mut self) -> Option<String> {
    // SAFETY: the display is live.
    let owner =
      unsafe { (self.x.get_selection_owner)(self.display, self.clipboard) };
    if owner == 0 {
      return None;
    }
    if owner == self.window {
      return self.owned.clone();
    }
    // SAFETY: the display and window are live.
    unsafe {
      (self.x.convert_selection)(
        self.display,
        self.clipboard,
        self.utf8,
        self.property,
        self.window,
        CURRENT_TIME,
      );
      (self.x.flush)(self.display);
    }
    let deadline = Instant::now() + READ_TIMEOUT;
    // SAFETY: the display is live.
    let x_fd = unsafe { (self.x.connection_number)(self.display) };
    let notify = loop {
      if let Some(notify) = self.handle_events(true) {
        break notify;
      }
      let left = deadline.saturating_duration_since(Instant::now());
      if left.is_zero() {
        return None;
      }
      let mut fds = [PollFd { fd: x_fd, events: POLLIN, revents: 0 }];
      // SAFETY: `fds` is a live array of one pollfd record.
      unsafe { poll(fds.as_mut_ptr(), 1, left.as_millis() as c_int) };
    };
    if notify.property == 0 {
      return None;
    }
    let mut kind: Atom = 0;
    let mut format: c_int = 0;
    let mut items: c_ulong = 0;
    let mut after: c_ulong = 0;
    let mut data: *mut u8 = std::ptr::null_mut();
    // SAFETY: the out-pointers are live locals. When XGetWindowProperty
    // gives data, it is `items` bytes (format 8), read once and freed with
    // XFree.
    unsafe {
      (self.x.get_window_property)(
        self.display,
        self.window,
        self.property,
        0,
        c_long::from(i32::MAX) / 4,
        1,
        0,
        &mut kind,
        &mut format,
        &mut items,
        &mut after,
        &mut data,
      );
      if data.is_null() {
        return None;
      }
      let text = (kind != self.incr && format == 8).then(|| {
        let bytes = std::slice::from_raw_parts(data, items as usize);
        String::from_utf8_lossy(bytes).into_owned()
      });
      (self.x.free)(data.cast());
      text
    }
  }
}

/// The system clipboard on X11.
pub struct X11Clipboard {
  commands: Sender<Command>,
  wake: c_int,
}

impl X11Clipboard {
  /// Starts the owner thread. `None` when libX11 or a display is missing.
  pub fn new() -> Option<X11Clipboard> {
    let mut fds = [0 as c_int; 2];
    // SAFETY: `fds` has room for the two descriptors pipe() writes.
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
      return None;
    }
    let (commands, receive) = mpsc::channel();
    let (ready, is_ready) = mpsc::channel();
    let wake_read = fds[0];
    thread::Builder::new()
      .name("dovetail clipboard".into())
      .spawn(move || match Owner::open() {
        Some(owner) => {
          let _ = ready.send(true);
          owner.run(receive, wake_read);
        },
        None => {
          let _ = ready.send(false);
        },
      })
      .ok()?;
    is_ready
      .recv()
      .ok()
      .filter(|&ok| ok)
      .map(|_| X11Clipboard { commands, wake: fds[1] })
  }

  fn send(
    &self,
    command: Command,
  ) -> bool {
    if self.commands.send(command).is_err() {
      return false;
    }
    // SAFETY: writes one byte from a live buffer to our own pipe.
    unsafe { write(self.wake, [1u8].as_ptr().cast(), 1) };
    true
  }
}

impl Clipboard for X11Clipboard {
  fn text(&self) -> Option<String> {
    let (reply, answer) = mpsc::channel();
    if !self.send(Command::Read(reply)) {
      return None;
    }
    answer
      .recv_timeout(READ_TIMEOUT * 2)
      .ok()
      .flatten()
  }

  fn set_text(
    &self,
    text: &str,
  ) -> bool {
    self.send(Command::Own(text.to_owned()))
  }
}
