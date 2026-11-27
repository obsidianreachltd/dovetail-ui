//! The self-test, in real windows (M19, `TestWindows`):
//!
//! - the main window is moved, resized and maximised, and a document
//!   window is moved and resized. Every window is closed, the application
//!   is started again, and each comes back where it was and in its state
//!   (criterion 1, as far as the system lets a program check it);
//! - the settings window blocks clicks, keys and closing in the main
//!   window until it closes (criterion 4);
//! - the full-screen command, with its shortcut, enters full screen and
//!   leaves it;
//! - with no windows left, the application ends, or keeps running on
//!   macOS, as it chose (criterion 5).
//!
//! It keeps its windows in a file of its own, which it removes at the end.
//! Each check has a time limit. Window positions are checked only where
//! the system gives them: not on Wayland.

use std::path::Path;
use std::process::ExitCode;

use dovetail::prelude::*;
use dovetail::{
  ExitPolicy, KeyCode, KeyEvent, Modifiers, NamedKey, Role, View, WindowState,
};
use dovetail_testing::windows::{TestWindow, TestWindows};
use dovetail_testing::{role, text};

use crate::MAIN;

fn button(label: &str) -> dovetail_testing::Find {
  role(Role::Button).label(label)
}

/// Within two pixels in each direction, which frames and rounding allow.
fn near(
  a: (f32, f32),
  b: (f32, f32),
) -> bool {
  (a.0 - b.0).abs() <= 2.0 && (a.1 - b.1).abs() <= 2.0
}

/// Starts the application with its windows kept in `file`.
fn start(
  t: &mut TestWindows,
  file: &Path,
) -> TestWindow {
  let file = file.to_path_buf();
  t.set_root(ui! { crate::App { saved_file: file } });
  t.window_titled(MAIN)
}

/// Takes the application away, which closes every window, and waits.
fn stop(t: &mut TestWindows) {
  t.set_root(View::Empty);
  t.wait_until("every window to close", |t| t.windows().is_empty());
}

/// Sizes `w` and moves it, and waits until it says it is there. Returns
/// its place then.
fn place(
  t: &mut TestWindows,
  w: TestWindow,
  position: (f32, f32),
  size: (f32, f32),
) -> dovetail::WindowPlace {
  t.set_size(w, size);
  t.wait_until("the size asked for", move |t| near(t.place(w).size, size));
  t.move_to(w, position);
  let positioned = t.place(w).position.is_some();
  if positioned {
    t.wait_until("the position asked for", move |t| {
      t.place(w)
        .position
        .is_some_and(|p| near(p, position))
    });
  }
  t.place(w)
}

fn full_screen_key() -> KeyEvent {
  if cfg!(target_os = "macos") {
    KeyEvent {
      key: KeyCode::Character("f".into()),
      modifiers: Modifiers {
        meta: true,
        control: true,
        ..Modifiers::default()
      },
      text: None,
      repeat: false,
    }
  } else {
    KeyEvent::named(NamedKey::F11)
  }
}

pub fn run() -> ExitCode {
  let file = std::env::temp_dir().join(format!(
    "dovetail-multi-window-self-test-{}.txt",
    std::process::id()
  ));
  let _ = std::fs::remove_file(&file);
  let mut t = TestWindows::new();
  t.with(|cx| cx.set_exit_policy(ExitPolicy::default()));

  // Several windows, moved, resized and maximised.
  let main = start(&mut t, &file);
  let main_was = place(&mut t, main, (120.0, 100.0), (700.0, 500.0));
  t.click(main, button("New document"));
  t.click(main, button("New document"));
  let one = t.window_titled("Document 1");
  t.window_titled("Document 2");
  let one_was = place(&mut t, one, (260.0, 220.0), (380.0, 260.0));
  t.set_state(main, WindowState::Maximised);
  println!(
    "PASS  three windows open, moved, resized and the main one maximised"
  );

  // Closed, and started again: each comes back as it was.
  stop(&mut t);
  let saved = std::fs::read_to_string(&file).expect("the windows were saved");
  assert!(saved.starts_with("dovetail-windows 1"), "{saved}");
  for name in [
    "\"main\"",
    "\"document 1\"",
    "\"document 2\"",
  ] {
    assert!(saved.contains(name), "{name} is saved:\n{saved}");
  }
  let main = start(&mut t, &file);
  t.wait_until("the main window to open maximised", move |t| {
    t.state(main) == WindowState::Maximised
  });
  // The label follows the state on the next rebuild.
  t.wait_until("the label to say Maximised", move |t| {
    t.has(main, text("Main window: Maximised"))
  });
  t.set_state(main, WindowState::Normal);
  t.wait_until(
    &format!("the main window's own place, {main_was:?}"),
    move |t| {
      let now = t.place(main);
      near(now.size, main_was.size)
        && match (now.position, main_was.position) {
          (Some(a), Some(b)) => near(a, b),
          _ => true,
        }
    },
  );
  t.click(main, button("New document"));
  let one = t.window_titled("Document 1");
  t.wait_until(&format!("document 1's own place, {one_was:?}"), move |t| {
    let now = t.place(one);
    near(now.size, one_was.size)
      && match (now.position, one_was.position) {
        (Some(a), Some(b)) => near(a, b),
        _ => true,
      }
  });
  t.request_close(one);
  t.wait_until("document 1 to close", move |t| !t.is_open(one));
  println!(
    "PASS  closed and opened again, each window came back at its size, \
     position and state"
  );

  // The settings window blocks the main window until it closes.
  t.click(main, button("Settings"));
  let settings = t.window_titled("Settings");
  assert_eq!(t.blocked_by(main), Some(settings));
  t.click(main, button("Click me"));
  t.find(main, text("Clicks: 0"));
  assert!(
    !t.key(main, NamedKey::Enter),
    "a key to the main window is refused"
  );
  t.request_close(main);
  t.pause(std::time::Duration::from_millis(200));
  assert!(t.is_open(main), "the main window cannot be closed");
  t.find(main, text("Clicks: 0"));
  t.click(settings, button("Done"));
  t.wait_until("the settings window to close", move |t| {
    !t.is_open(settings)
  });
  assert_eq!(t.blocked_by(main), None);
  t.click(main, button("Click me"));
  t.find(main, text("Clicks: 1"));
  println!(
    "PASS  the settings window blocked clicks, keys and closing in its owner"
  );

  // Full screen with the command's shortcut, and back.
  // A click gives the keyboard to the window's content.
  t.click(main, button("Click me"));
  assert!(
    t.press(main, full_screen_key()),
    "the shortcut ran the command"
  );
  t.wait_until("full screen", move |t| {
    t.state(main) == WindowState::FullScreen
  });
  // The label follows the state on the next rebuild.
  t.wait_until("the label to say FullScreen", move |t| {
    t.has(main, text("Main window: FullScreen"))
  });
  assert!(t.press(main, full_screen_key()));
  t.wait_until("the normal state again", move |t| {
    t.state(main) == WindowState::Normal
  });
  println!("PASS  the full-screen shortcut entered full screen and left it");

  // With no windows left, it ends or keeps running, as it chose.
  stop(&mut t);
  let ends = t.with(|cx| cx.would_end());
  assert_eq!(ends, !cfg!(target_os = "macos"));
  println!(
    "PASS  with no windows left the application {}",
    if ends {
      "ends"
    } else {
      "keeps running, as on macOS"
    }
  );

  let _ = std::fs::remove_file(&file);
  println!(
    "PASS  several windows, their saved state, a modal window and full screen work"
  );
  ExitCode::SUCCESS
}
