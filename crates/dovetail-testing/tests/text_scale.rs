//! M18 step 8: the system's text size grows the theme's text, so every
//! control's text and plain text inside the outermost theme grow, while
//! sizes an application sets stay as they are.

use dovetail::prelude::*;
use dovetail::theme::{Theme, ThemeData};
use dovetail_core::{SystemSettings, set_system_settings};
use dovetail_layout::Content;
use dovetail_testing::TestUi;

#[component]
fn Page() -> View {
  ui! {
    Theme {
      data: ThemeData::light(),
      Column {
        Text { value: "Plain" }
        Text { value: "Fixed", size: Some(24.0) }
        Button { text: "Press" }
      }
    }
  }
}

/// The font size of the text that says `words`.
fn size_of(
  ui: &TestUi,
  words: &str,
) -> f32 {
  ui.layout()
    .boxes()
    .iter()
    .find_map(|b| match &b.content {
      Content::Text { text, font_size, .. } if &**text == words => {
        Some(*font_size)
      },
      _ => None,
    })
    .unwrap_or_else(|| panic!("no text {words:?}"))
}

#[test]
fn the_theme_text_follows_the_system_text_size() {
  let mut ui = TestUi::new(ui! { Page {} });
  let text = ThemeData::light().text;
  assert_eq!(size_of(&ui, "Plain"), text.body.size);
  assert_eq!(size_of(&ui, "Press"), text.label.size);
  assert_eq!(size_of(&ui, "Fixed"), 24.0);
  let before = ui.find(dovetail_testing::label("Press")).rect();

  set_system_settings(SystemSettings {
    text_scale: 1.5,
    ..SystemSettings::default()
  });
  ui.update();
  assert_eq!(size_of(&ui, "Plain"), text.body.size * 1.5);
  assert_eq!(size_of(&ui, "Press"), text.label.size * 1.5);
  assert_eq!(size_of(&ui, "Fixed"), 24.0, "a size the application set");
  let after = ui.find(dovetail_testing::label("Press")).rect();
  assert!(after.height > before.height, "the button grows to fit");

  set_system_settings(SystemSettings::default());
  ui.update();
  assert_eq!(size_of(&ui, "Press"), text.label.size);
}
