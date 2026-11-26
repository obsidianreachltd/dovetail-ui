//! M20 criterion 5: a painted element is compared with a saved picture,
//! and the difference is saved when it fails.
//!
//! These need a GPU adapter, so they do not run by default:
//! `cargo test -p dovetail-testing --features pictures -- --ignored`.
//! `DOVETAIL_UPDATE_PICTURES=1` saves the pictures in `tests/pictures/`
//! again.

#![cfg(feature = "pictures")]

use dovetail::prelude::*;
use dovetail_core::Role;
use dovetail_testing::{Options, TestUi, label, role};

#[component]
fn Counter(start: i32) -> View {
  let count = use_state(move || start);
  ui! {
    Column {
      Text { value: count.map(|n| format!("Count: {n}")) }
      Button { text: "Increment", on_click: move |_| count.update(|n| *n += 1) }
    }
  }
}

fn small(start: i32) -> TestUi {
  TestUi::with(
    ui! { Counter { start: start } },
    Options {
      size: (240.0, 120.0),
      ..Options::default()
    },
  )
}

#[test]
#[ignore = "needs a GPU adapter"]
fn a_button_matches_its_saved_picture() {
  let ui = small(0);
  let button = ui.find(role(Role::Button).label("Increment"));
  ui.assert_picture(&button, "increment-button");
}

#[test]
#[ignore = "needs a GPU adapter"]
fn a_changed_picture_fails_and_saves_the_difference() {
  // The saved picture shows "Count: 0".
  let ui = small(0);
  let count = ui.find(role(Role::Label));
  ui.assert_picture(&count, "count-zero");
  // Saving pictures again would save the changed one over it.
  if std::env::var_os("DOVETAIL_UPDATE_PICTURES").is_some() {
    return;
  }

  // "Count: 1" is the same size, with one letter different.
  let ui = small(1);
  let count = ui.find(role(Role::Label));
  let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    ui.assert_picture(&count, "count-zero");
  }))
  .expect_err("the picture changed");
  let message = failure
    .downcast_ref::<String>()
    .cloned()
    .unwrap_or_default();
  assert!(
    message.contains("pixels in the picture \"count-zero\" differ"),
    "{message}"
  );
  let saved = |end: &str| {
    message
      .split(' ')
      .map(|w| w.trim_end_matches([',', '.']))
      .find(|w| w.ends_with(end))
      .map(std::path::PathBuf::from)
      .unwrap_or_else(|| panic!("no {end} in: {message}"))
  };
  let now = saved("count-zero.png");
  let difference = saved("count-zero.difference.png");
  assert!(now.is_file(), "{}", now.display());
  assert!(difference.is_file(), "{}", difference.display());
  // Only the changed digit is red.
  let diff = dovetail_testing::Picture::from_png(
    &std::fs::read(&difference).expect("the difference"),
  )
  .expect("a PNG");
  let red = diff
    .pixels
    .iter()
    .filter(|p| **p == [255, 0, 0, 255])
    .count();
  assert!(red > 0 && red < diff.pixels.len() / 4, "{red} red pixels");
}

#[test]
#[ignore = "needs a GPU adapter"]
fn pictures_are_painted_at_the_windows_scale() {
  let one = small(0);
  let two = TestUi::with(
    ui! { Counter { start: 0 } },
    Options {
      size: (240.0, 120.0),
      scale: 2.0,
      ..Options::default()
    },
  );
  let a = one.picture(&one.find(label("Increment")));
  let b = two.picture(&two.find(label("Increment")));
  // Twice the size, give or take the pixel an edge not on a whole pixel
  // rounds to.
  let near = |x: u32, y: u32| x.abs_diff(y) <= 1;
  assert!(
    near(b.width, a.width * 2) && near(b.height, a.height * 2),
    "{} by {} at 2, {} by {} at 1",
    b.width,
    b.height,
    a.width,
    a.height
  );
  let window = two.window_picture();
  assert_eq!((window.width, window.height), (480, 240));
}
