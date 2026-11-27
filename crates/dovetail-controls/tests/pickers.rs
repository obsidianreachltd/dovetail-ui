//! M25 "Done when" items 2, 5 and 6: toggle buttons, button groups, and
//! date, time and colour pickers, used with the pointer, with the keyboard
//! alone and by typing, found by role and label, with no window or GPU.

extern crate dovetail_core as dovetail;

use dovetail::{
  Color, Date, KeyEvent, Modifiers, NamedKey, Role, Time, View, use_state,
};
use dovetail_controls::{
  ButtonGroup, Choose, ColourButton, ColourPicker, Column, DateField,
  GroupButton, Text, TimeField, ToggleButton,
};
use dovetail_macros::{component, ui};
use dovetail_testing::{TestUi, label, role, text};
use dovetail_theme::color::to_hex;

fn date(
  y: i32,
  m: u32,
  d: u32,
) -> Date {
  Date::new(y, m, d).unwrap()
}

#[component]
fn Toggles() -> View {
  let bold = use_state(|| false);
  let align = use_state(|| vec![0usize]);
  let styles = use_state(Vec::<usize>::new);
  let pressed = use_state(|| None::<usize>);
  ui! {
    Column {
      Text { value: format!("bold {}", bold.get()) }
      Text { value: format!("align {:?}", align.get()) }
      Text { value: format!("styles {:?}", styles.get()) }
      Text { value: format!("pressed {:?}", pressed.get()) }
      ToggleButton { on: bold.get(), on_change: move |on| bold.set(on), text: "Bold" }
      ButtonGroup {
        buttons: vec![GroupButton::text("Left"), GroupButton::text("Centre"), GroupButton::text("Right")],
        chosen: align.get(),
        on_press: move |i| align.set(vec![i]),
        label: "Alignment",
      }
      ButtonGroup {
        buttons: vec![GroupButton::text("Italic"), GroupButton::text("Underline")],
        chosen: styles.get(),
        choose: Choose::Many,
        on_press: move |i| styles.update(|s| if let Some(p) = s.iter().position(|&x| x == i) { s.remove(p); } else { s.push(i); s.sort(); }),
        label: "Styles",
      }
      ButtonGroup {
        buttons: vec![GroupButton::text("Cut"), GroupButton::text("Copy").enabled(false), GroupButton::text("Paste")],
        choose: Choose::None,
        on_press: move |i| pressed.set(Some(i)),
        label: "Clipboard",
      }
    }
  }
}

#[test]
fn a_toggle_button_stays_pressed_until_pressed_again() {
  let mut ui = TestUi::new(ui! { Toggles {} });
  let bold = role(Role::Button).label("Bold");
  assert_eq!(ui.find(bold.clone()).is_on(), Some(false));
  ui.click(bold.clone());
  assert!(ui.has(text("bold true")));
  assert_eq!(ui.find(bold.clone()).is_on(), Some(true), "read as pressed");
  ui.focus(bold.clone());
  ui.key(NamedKey::Space);
  assert!(ui.has(text("bold false")), "Space presses it too");
}

#[test]
fn a_group_with_one_choice_moves_it_with_the_arrow_keys() {
  let mut ui = TestUi::new(ui! { Toggles {} });
  assert_eq!(ui.find(role(Role::RadioGroup)).label(), Some("Alignment"));
  ui.click(role(Role::RadioButton).label("Right"));
  assert!(ui.has(text("align [2]")));
  assert_eq!(
    ui.find(role(Role::RadioButton).label("Right"))
      .is_on(),
    Some(true)
  );
  // One Tab stop: the chosen button.
  let focusable: Vec<_> = ui
    .find_all(role(Role::RadioButton))
    .into_iter()
    .filter(|f| f.node().actions.focus)
    .collect();
  assert_eq!(focusable.len(), 1);
  ui.focus(role(Role::RadioButton).label("Right"));
  ui.key(NamedKey::ArrowRight);
  assert!(ui.has(text("align [0]")), "it goes round");
  ui.key(NamedKey::End);
  assert!(ui.has(text("align [2]")));
  ui.key(NamedKey::ArrowLeft);
  assert!(ui.has(text("align [1]")));
  assert_eq!(ui.focused().unwrap().label(), Some("Centre"));
}

#[test]
fn a_group_of_several_choices_or_none_moves_focus_and_presses() {
  let mut ui = TestUi::new(ui! { Toggles {} });
  ui.click(role(Role::Button).label("Underline"));
  ui.click(role(Role::Button).label("Italic"));
  assert!(ui.has(text("styles [0, 1]")), "{}", ui.describe());
  ui.click(role(Role::Button).label("Italic"));
  assert!(ui.has(text("styles [1]")));
  assert_eq!(
    ui.find(role(Role::Button).label("Underline"))
      .is_on(),
    Some(true)
  );

  // Plain buttons have no state, and skip the disabled one.
  assert_eq!(ui.find(role(Role::Button).label("Cut")).is_on(), None);
  assert!(
    ui.find(role(Role::Button).label("Copy"))
      .is_disabled()
  );
  ui.focus(role(Role::Button).label("Cut"));
  ui.key(NamedKey::ArrowRight);
  assert_eq!(ui.focused().unwrap().label(), Some("Paste"));
  ui.key(NamedKey::Enter);
  assert!(ui.has(text("pressed Some(2)")));
}

#[component]
fn Dates() -> View {
  let day = use_state(|| Some(date(2026, 1, 20)));
  ui! {
    Column {
      Text { value: format!("day {:?}", day.get().map(|d| d.to_string())) }
      DateField {
        value: day.get(),
        on_change: move |d| day.set(Some(d)),
        first: Some(date(2026, 1, 5)),
        last: Some(date(2026, 3, 31)),
        label: "Start",
      }
    }
  }
}

#[test]
fn a_date_can_be_typed_and_one_out_of_range_is_refused() {
  let mut ui = TestUi::new(ui! { Dates {} });
  let field = role(Role::TextInput).label("Start");
  assert_eq!(ui.find(field.clone()).value(), Some("2026-01-20"));
  assert_eq!(
    ui.find(role(Role::DateInput)).value(),
    Some("2026-01-20"),
    "read as a date"
  );
  ui.focus(field.clone());
  ui.shortcut('a');
  ui.type_text("2026-2-14\n");
  assert!(ui.has(text("day Some(\"2026-02-14\")")));

  ui.shortcut('a');
  ui.type_text("2027-01-01\n");
  assert!(ui.has(text("day Some(\"2026-02-14\")")), "kept");
  assert!(ui.has(text("Choose a date from 2026-01-05 to 2026-03-31.")));
  ui.shortcut('a');
  ui.type_text("next week\n");
  assert!(ui.has(text("Type a date as year-month-day.")));
}

#[test]
fn the_calendar_opens_and_chooses_with_the_keyboard_alone() {
  let mut ui = TestUi::new(ui! { Dates {} });
  ui.focus(role(Role::TextInput).label("Start"));
  ui.press(
    KeyEvent::named(NamedKey::ArrowDown)
      .with(Modifiers { alt: true, ..Modifiers::default() }),
  );
  let grid = ui.find(role(Role::Grid));
  assert_eq!(grid.label(), Some("Start"));
  assert!(
    ui.focused()
      .is_some_and(|f| f.role() == Role::Grid)
  );
  assert!(ui.has(text("January 2026")));
  let chosen = ui.find(role(Role::GridCell).label("Tuesday 20 January 2026"));
  assert_eq!(chosen.node().selected, Some(true));
  // Today, from the test's date, is marked.
  let today = ui.find(role(Role::GridCell).label("Thursday 15 January 2026"));
  assert_eq!(today.node().description.as_deref(), Some("Today"));

  ui.key(NamedKey::ArrowDown);
  ui.key(NamedKey::PageDown);
  assert!(ui.has(text("February 2026")));
  ui.key(NamedKey::Home);
  ui.key(NamedKey::Enter);
  // 27 January, a month on is 27 February, a Friday; Home goes to Monday.
  assert!(ui.has(text("day Some(\"2026-02-23\")")));
  assert!(!ui.has(role(Role::Grid)), "choosing closes it");
}

#[test]
fn the_calendar_keeps_to_its_dates_and_chooses_with_the_pointer() {
  let mut ui = TestUi::new(ui! { Dates {} });
  ui.click(role(Role::Button).label("Choose a date"));
  let early = ui.find(role(Role::GridCell).label("Thursday 1 January 2026"));
  assert!(early.is_disabled(), "before the first date");
  assert!(
    ui.find(role(Role::Button).label("Previous month"))
      .is_disabled()
  );
  ui.click(role(Role::GridCell).label("Wednesday 28 January 2026"));
  assert!(ui.has(text("day Some(\"2026-01-28\")")));

  ui.click(role(Role::Button).label("Choose a date"));
  ui.key(NamedKey::Escape);
  assert!(!ui.has(role(Role::Grid)), "Escape closes it");
}

#[component]
fn Times() -> View {
  let time = use_state(|| Time::new(9, 30, 0).unwrap());
  let late = use_state(|| Time::new(23, 15, 0).unwrap());
  ui! {
    Column {
      Text { value: format!("time {}", time.get()) }
      Text { value: format!("late {}", late.get()) }
      TimeField { value: time.get(), on_change: move |t| time.set(t), label: "Alarm" }
      TimeField { value: late.get(), on_change: move |t| late.set(t), twelve_hour: true, seconds: true, label: "Late" }
    }
  }
}

#[test]
fn a_time_moves_by_its_parts_and_refuses_numbers_out_of_range() {
  let mut ui = TestUi::new(ui! { Times {} });
  let alarm = role(Role::TimeInput).label("Alarm");
  assert_eq!(ui.find(alarm.clone()).value(), Some("09:30"));
  ui.focus(alarm.clone());
  ui.key(NamedKey::ArrowUp);
  assert!(ui.has(text("time 10:30")));
  ui.key(NamedKey::ArrowRight);
  ui.key(NamedKey::ArrowDown);
  assert!(ui.has(text("time 10:29")));
  // Typed: 7 then 5 is 75, out of range, so the 5 starts again.
  ui.type_text("75");
  assert!(ui.has(text("time 10:05")), "{}", ui.describe());
  ui.key(NamedKey::Home);
  ui.type_text("18");
  assert!(ui.has(text("time 18:05")));

  // With the pointer: a click on a part, then the buttons step it.
  let field = ui.find(role(Role::Group).label("Alarm"));
  let later = ui.find_within(&field, role(Role::Button).label("Later"));
  ui.click(text("05"));
  ui.click_at(later.centre());
  assert!(ui.has(text("time 18:06")), "{}", ui.describe());
}

#[test]
fn a_twelve_hour_time_has_a_half_of_the_day() {
  let mut ui = TestUi::new(ui! { Times {} });
  let late = role(Role::TimeInput).label("Late");
  assert_eq!(ui.find(late.clone()).value(), Some("11:15:00 PM"));
  ui.focus(late.clone());
  ui.key(NamedKey::End);
  ui.type_text("a");
  assert!(ui.has(text("late 11:15")));
  ui.key(NamedKey::Home);
  ui.type_text("13");
  assert!(
    ui.has(text("late 03:15")),
    "13 is refused: the 3 starts again"
  );
}

#[component]
fn Colours() -> View {
  let colour = use_state(|| Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
  ui! {
    Column {
      Text { value: format!("colour {}", to_hex(colour.get())) }
      ColourPicker {
        value: colour.get(),
        on_change: move |c| colour.set(c),
        swatches: vec![Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }],
      }
    }
  }
}

#[test]
fn a_colour_can_be_typed_moved_with_keys_and_chosen_from_swatches() {
  let mut ui = TestUi::new(ui! { Colours {} });
  let hex = role(Role::TextInput).label("Hex");
  ui.focus(hex.clone());
  ui.shortcut('a');
  ui.type_text("#00ff00\n");
  assert!(ui.has(text("colour #00ff00")));
  ui.shortcut('a');
  ui.type_text("green\n");
  assert!(ui.has(text("Type a colour as #rrggbb.")));
  assert!(ui.has(text("colour #00ff00")), "refused");

  // The colour area: Left lowers the saturation.
  ui.focus(role(Role::Slider).label("Saturation and brightness"));
  ui.press(
    KeyEvent::named(NamedKey::ArrowLeft)
      .with(Modifiers { shift: true, ..Modifiers::default() }),
  );
  assert!(ui.has(text("colour #1aff1a")), "{}", ui.describe());

  // Hue: End goes round to the end.
  ui.focus(role(Role::Slider).label("Hue"));
  ui.key(NamedKey::Home);
  assert!(ui.has(text("colour #ff1a1a")));
  // Opacity.
  ui.focus(role(Role::Slider).label("Opacity"));
  ui.key(NamedKey::Home);
  assert!(ui.has(text("colour #ff1a1a00")));

  ui.click(role(Role::Button).label("#0000ff"));
  assert!(ui.has(text("colour #0000ff")));
  // The RGB fields follow.
  let blue = ui.find(role(Role::SpinButton).label("B"));
  assert!(format!("{:?}", blue.node().numeric).contains("value: 255.0"));
}

#[test]
fn a_colour_button_opens_the_picker_as_a_layer() {
  let mut ui = TestUi::new(ui! {
    ColourButton { value: Color { r: 0.0, g: 0.5, b: 1.0, a: 1.0 }, label: "Fill" }
  });
  let well = ui.find(role(Role::ColourWell).label("Fill"));
  assert_eq!(well.value(), Some("#0080ff"));
  assert!(!ui.has(label("Hue")));
  ui.click(role(Role::ColourWell));
  assert!(ui.has(role(Role::Slider).label("Hue")));
  ui.focus(role(Role::Slider).label("Hue"));
  ui.key(NamedKey::Escape);
  assert!(!ui.has(label("Hue")), "Escape closes it");
}
