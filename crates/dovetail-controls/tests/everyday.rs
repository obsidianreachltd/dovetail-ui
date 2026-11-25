//! M15 "Done when" item 2 and 3: check boxes, radio groups, drop-downs,
//! number fields, sliders, tabs, collapsible sections, links and tooltips,
//! used with the pointer and with the keyboard. These lay the controls out
//! and drive their handlers, with no window or GPU.

extern crate dovetail_core as dovetail;

use std::time::{Duration, Instant};

use dovetail::{
  AccessAction, KeyEvent, Modifiers, NamedKey, PointerButton, PointerEvent,
  PointerKind, Role, Tree, View, run_task_messages, use_state,
};
use dovetail_controls::{
  CheckBox, Collapsible, Column, Link, NumberField, RadioGroup, Select, Slider,
  Tabs, Text, Tooltip,
};
use dovetail_layout::{BoxKey, Content, FixedPitch, Layout};
use dovetail_macros::{component, ui};

fn options(names: &[&str]) -> Vec<String> {
  names.iter().map(|s| s.to_string()).collect()
}

/// Every M15 control, with its state shown as text so tests can read it.
#[component]
fn Panel() -> View {
  let checked = use_state(|| false);
  let mixed = use_state(|| true);
  let radio = use_state(|| Some(0usize));
  let choice = use_state(|| None::<usize>);
  let number = use_state(|| 5.0f32);
  let level = use_state(|| 50.0f32);
  let tab = use_state(|| 0usize);
  let open = use_state(|| false);
  let links = use_state(|| 0u32);
  ui! {
    Column {
      Text { value: format!("checked {}", checked.get()) }
      Text { value: format!("mixed {}", mixed.get()) }
      Text { value: format!("radio {:?}", radio.get()) }
      Text { value: format!("choice {:?}", choice.get()) }
      Text { value: format!("number {}", number.get()) }
      Text { value: format!("level {}", level.get()) }
      Text { value: format!("tab {}", tab.get()) }
      Text { value: format!("links {}", links.get()) }
      CheckBox { checked: checked.get(), on_change: move |on| checked.set(on), label: "Wrap lines" }
      CheckBox { checked: false, mixed: mixed.get(), on_change: move |on| mixed.set(!on), label: "All" }
      CheckBox { checked: true, enabled: false, label: "Locked" }
      RadioGroup {
        options: options(&["Small", "Medium", "Large"]),
        selected: radio.get(),
        on_change: move |i| radio.set(Some(i)),
        label: "Size",
      }
      Select {
        options: options(&["Apple", "Banana", "Blueberry", "Cherry"]),
        selected: choice.get(),
        on_change: move |i| choice.set(Some(i)),
        label: "Fruit",
        placeholder: "Choose",
      }
      NumberField { value: number.get(), on_change: move |v| number.set(v), min: 0.0, max: 10.0, label: "Copies" }
      Slider { value: level.get(), on_change: move |v| level.set(v), step: 5.0, width: 216.0, label: "Volume" }
      Tabs {
        tabs: options(&["General", "Advanced"]),
        selected: tab.get(),
        on_change: move |i| tab.set(i),
        Text { value: format!("panel {}", tab.get()) }
      }
      Collapsible {
        title: "More",
        open: open.get(),
        on_toggle: move |o| open.set(o),
        Text { value: "hidden part" }
      }
      Link { text: "Help", on_click: move |_| links.update(|n| *n += 1) }
      Tooltip {
        text: "Saves the file",
        CheckBox { checked: false, label: "Save" }
      }
    }
  }
}

struct Window {
  tree: Tree,
  layout: Layout,
}

impl Window {
  fn open() -> Self {
    let mut tree = Tree::new();
    tree.mount(ui! { Panel {} });
    let mut window = Self { tree, layout: Layout::new() };
    window.refresh();
    window
  }

  fn refresh(&mut self) {
    self.tree.update();
    self
      .layout
      .update(&self.tree, 800.0, 1200.0, &FixedPitch::default());
  }

  /// The text that starts with `start`.
  fn shown(
    &self,
    start: &str,
  ) -> String {
    self
      .layout
      .boxes()
      .iter()
      .find_map(|b| match &b.content {
        Content::Text { text, .. } if text.starts_with(start) => {
          Some(text.to_string())
        },
        _ => None,
      })
      .unwrap_or_else(|| panic!("no text starting {start:?}"))
  }

  fn has_text(
    &self,
    text: &str,
  ) -> bool {
    self.layout.boxes().iter().any(
      |b| matches!(&b.content, Content::Text { text: t, .. } if &**t == text),
    )
  }

  /// The elements with `role`, and `label` when it is not empty.
  fn all(
    &self,
    role: Role,
    label: &str,
  ) -> Vec<(BoxKey, dovetail::Element)> {
    self
      .layout
      .boxes()
      .iter()
      .filter_map(|b| match &b.content {
        Content::Element(e)
          if e.role == role
            && (label.is_empty() || e.label.as_deref() == Some(label)) =>
        {
          Some((b.key.clone(), (**e).clone()))
        },
        _ => None,
      })
      .collect()
  }

  fn one(
    &self,
    role: Role,
    label: &str,
  ) -> (BoxKey, dovetail::Element) {
    self
      .all(role, label)
      .into_iter()
      .next()
      .unwrap_or_else(|| panic!("no {role:?} {label:?}"))
  }

  fn press(
    &mut self,
    role: Role,
    label: &str,
  ) {
    let (_, e) = self.one(role, label);
    e.on_press
      .unwrap_or_else(|| panic!("{role:?} {label:?} cannot be pressed"))
      .call(());
    self.refresh();
  }

  /// A key, sent as if `key` had focus. True when something used it.
  fn key_at(
    &mut self,
    key: &BoxKey,
    event: KeyEvent,
  ) -> bool {
    let used = self.layout.dispatch_key(Some(key), &event);
    self.refresh();
    used
  }

  fn key(
    &mut self,
    role: Role,
    label: &str,
    event: KeyEvent,
  ) -> bool {
    let (key, _) = self.one(role, label);
    self.key_at(&key, event)
  }

  fn access(
    &mut self,
    role: Role,
    label: &str,
    action: AccessAction,
  ) {
    let (_, e) = self.one(role, label);
    e.on_access
      .expect("takes actions from tools")
      .call(action);
    self.refresh();
  }
}

fn named(key: NamedKey) -> KeyEvent {
  KeyEvent::named(key)
}

#[test]
fn a_check_box_turns_on_and_off_and_a_mixed_one_turns_on() {
  let mut w = Window::open();
  let (_, e) = w.one(Role::CheckBox, "Wrap lines");
  assert_eq!(
    (e.toggled, e.mixed, e.focusable),
    (Some(false), false, true)
  );
  w.press(Role::CheckBox, "Wrap lines");
  assert_eq!(w.shown("checked"), "checked true");
  // Space, with focus on it, presses it again.
  assert!(w.key(Role::CheckBox, "Wrap lines", named(NamedKey::Space)));
  assert_eq!(w.shown("checked"), "checked false");

  let (_, all) = w.one(Role::CheckBox, "All");
  assert!(all.mixed);
  w.press(Role::CheckBox, "All");
  assert_eq!(w.shown("mixed"), "mixed false");

  let (_, locked) = w.one(Role::CheckBox, "Locked");
  assert!(locked.disabled && !locked.focusable && locked.on_press.is_none());
}

#[test]
fn a_radio_group_moves_its_choice_with_the_pointer_and_the_arrows() {
  let mut w = Window::open();
  let buttons = w.all(Role::RadioButton, "");
  assert_eq!(buttons.len(), 3);
  // Only the chosen button is a Tab stop.
  let stops: Vec<bool> = buttons.iter().map(|(_, e)| e.focusable).collect();
  assert_eq!(stops, [true, false, false]);
  let large = buttons[2].1.on_press.clone().expect("pressable");
  large.call(());
  w.refresh();
  assert_eq!(w.shown("radio"), "radio Some(2)");
  // Down from the last goes round to the first; Up goes back.
  let chosen = w.all(Role::RadioButton, "")[2].0.clone();
  assert!(w.key_at(&chosen, named(NamedKey::ArrowDown)));
  assert_eq!(w.shown("radio"), "radio Some(0)");
  let chosen = w.all(Role::RadioButton, "")[0].0.clone();
  assert!(w.key_at(&chosen, named(NamedKey::ArrowUp)));
  assert_eq!(w.shown("radio"), "radio Some(2)");
  let toggled: Vec<Option<bool>> = w
    .all(Role::RadioButton, "")
    .iter()
    .map(|(_, e)| e.toggled)
    .collect();
  assert_eq!(toggled, [Some(false), Some(false), Some(true)]);
}

#[test]
fn a_drop_down_opens_and_chooses_with_the_pointer() {
  let mut w = Window::open();
  let (_, combo) = w.one(Role::ComboBox, "Fruit");
  assert_eq!(combo.expanded, Some(false));
  assert!(w.has_text("Choose"));
  w.press(Role::ComboBox, "Fruit");
  assert_eq!(w.one(Role::ComboBox, "Fruit").1.expanded, Some(true));
  assert_eq!(w.all(Role::ListBoxOption, "").len(), 4);
  let cherry = w.all(Role::ListBoxOption, "")[3].1.clone();
  cherry.on_press.expect("pressable").call(());
  w.refresh();
  assert_eq!(w.shown("choice"), "choice Some(3)");
  assert!(w.all(Role::ListBox, "").is_empty(), "the list closed");
  assert_eq!(
    w.one(Role::ComboBox, "Fruit")
      .1
      .text_value
      .as_deref(),
    Some("Cherry")
  );
}

#[test]
fn a_drop_down_works_from_the_keyboard_alone() {
  let mut w = Window::open();
  // Closed: Down chooses the first, letters jump and go round.
  assert!(w.key(Role::ComboBox, "Fruit", named(NamedKey::ArrowDown)));
  assert_eq!(w.shown("choice"), "choice Some(0)");
  assert!(w.key(Role::ComboBox, "Fruit", KeyEvent::character('b')));
  assert_eq!(w.shown("choice"), "choice Some(1)");
  assert!(w.key(Role::ComboBox, "Fruit", KeyEvent::character('b')));
  assert_eq!(w.shown("choice"), "choice Some(2)");
  // Enter opens; Down and Enter choose; Escape closes without choosing.
  assert!(w.key(Role::ComboBox, "Fruit", named(NamedKey::Enter)));
  assert!(!w.all(Role::ListBox, "Fruit").is_empty());
  assert!(w.key(Role::ComboBox, "Fruit", named(NamedKey::ArrowDown)));
  assert!(w.key(Role::ComboBox, "Fruit", named(NamedKey::Enter)));
  assert_eq!(w.shown("choice"), "choice Some(3)");
  assert!(w.all(Role::ListBox, "").is_empty());
  assert!(w.key(Role::ComboBox, "Fruit", named(NamedKey::Space)));
  assert!(w.key(Role::ComboBox, "Fruit", named(NamedKey::ArrowUp)));
  assert!(w.key(Role::ComboBox, "Fruit", named(NamedKey::Escape)));
  assert_eq!(w.shown("choice"), "choice Some(3)");
  assert!(w.all(Role::ListBox, "").is_empty());
}

#[test]
fn a_number_field_steps_with_keys_buttons_and_tools_within_its_range() {
  let mut w = Window::open();
  let (_, spin) = w.one(Role::SpinButton, "Copies");
  assert_eq!(spin.value, Some(5.0));
  let range = spin.range.expect("a range");
  assert_eq!((range.min, range.max, range.step), (0.0, 10.0, 1.0));
  // Up in the text field inside goes to the number field.
  assert!(w.key(Role::TextInput, "Copies", named(NamedKey::ArrowUp)));
  assert_eq!(w.shown("number"), "number 6");
  assert!(w.key(Role::TextInput, "Copies", named(NamedKey::PageUp)));
  assert_eq!(w.shown("number"), "number 10", "kept inside the range");
  w.press(Role::Button, "Decrease");
  assert_eq!(w.shown("number"), "number 9");
  w.access(Role::SpinButton, "Copies", AccessAction::SetNumber(2.4));
  assert_eq!(w.shown("number"), "number 2", "on a whole step");
  w.access(Role::SpinButton, "Copies", AccessAction::Increment);
  assert_eq!(w.shown("number"), "number 3");
  // Typing a number in range changes it; the field shows the new value.
  w.access(
    Role::TextInput,
    "Copies",
    AccessAction::SetValue("7".into()),
  );
  assert_eq!(w.shown("number"), "number 7");
  assert!(w.has_text("7"));
  // Out of range is not taken.
  w.access(
    Role::TextInput,
    "Copies",
    AccessAction::SetValue("70".into()),
  );
  assert_eq!(w.shown("number"), "number 7");
}

#[test]
fn a_slider_moves_with_the_keys_and_the_pointer() {
  let mut w = Window::open();
  assert!(w.key(Role::Slider, "Volume", named(NamedKey::ArrowRight)));
  assert_eq!(w.shown("level"), "level 55");
  assert!(w.key(Role::Slider, "Volume", named(NamedKey::PageDown)));
  assert_eq!(w.shown("level"), "level 5");
  assert!(w.key(Role::Slider, "Volume", named(NamedKey::End)));
  assert_eq!(w.shown("level"), "level 100");
  assert!(w.key(Role::Slider, "Volume", named(NamedKey::Home)));
  assert_eq!(w.shown("level"), "level 0");
  // A press a quarter of the way along the 200 pixels the thumb moves.
  let (_, slider) = w.one(Role::Slider, "Volume");
  slider
    .on_pointer
    .expect("takes the pointer")
    .handle(&PointerEvent {
      kind: PointerKind::Down,
      button: PointerButton::Primary,
      x: 8.0 + 50.0,
      y: 10.0,
      window_x: 0.0,
      window_y: 0.0,
      text_at: None,
      modifiers: Modifiers::default(),
      clicks: 1,
    });
  w.refresh();
  assert_eq!(w.shown("level"), "level 25");
  let (_, slider) = w.one(Role::Slider, "Volume");
  assert_eq!(slider.value, Some(25.0));
}

#[test]
fn tabs_choose_with_the_pointer_and_the_arrows() {
  let mut w = Window::open();
  assert!(w.has_text("panel 0"));
  let tabs = w.all(Role::Tab, "");
  tabs[1]
    .1
    .on_press
    .clone()
    .expect("pressable")
    .call(());
  w.refresh();
  assert_eq!(w.shown("tab"), "tab 1");
  assert!(w.has_text("panel 1"));
  let chosen = w.all(Role::Tab, "")[1].0.clone();
  assert!(w.key_at(&chosen, named(NamedKey::ArrowRight)));
  assert_eq!(w.shown("tab"), "tab 0");
  assert_eq!(w.one(Role::TabPanel, "General").1.role, Role::TabPanel);
}

#[test]
fn a_collapsible_section_opens_and_a_link_presses() {
  let mut w = Window::open();
  assert!(!w.has_text("hidden part"));
  let (_, header) = w.one(Role::Button, "More");
  assert_eq!(header.expanded, Some(false));
  assert!(w.key(Role::Button, "More", named(NamedKey::Enter)));
  assert!(w.has_text("hidden part"));
  assert_eq!(w.one(Role::Button, "More").1.expanded, Some(true));

  w.press(Role::Link, "");
  assert!(w.key(Role::Link, "", named(NamedKey::Enter)));
  assert_eq!(w.shown("links"), "links 2");
}

/// The tooltip wrapper: the element with a description.
fn tip_wrapper(w: &Window) -> dovetail::Element {
  w.layout
    .boxes()
    .iter()
    .find_map(|b| match &b.content {
      Content::Element(e) if e.description.is_some() => Some((**e).clone()),
      _ => None,
    })
    .expect("the tooltip's wrapper")
}

#[test]
fn a_tooltip_shows_on_focus_at_once_and_after_a_pause_on_hover() {
  let mut w = Window::open();
  let wrapper = tip_wrapper(&w);
  assert_eq!(wrapper.description.as_deref(), Some("Saves the file"));
  assert!(w.all(Role::Tooltip, "").is_empty());

  wrapper
    .on_focus_within
    .clone()
    .expect("focus")
    .call(true);
  w.refresh();
  assert!(w.has_text("Saves the file"));
  assert_eq!(w.all(Role::Tooltip, "").len(), 1);
  wrapper
    .on_focus_within
    .clone()
    .expect("focus")
    .call(false);
  w.refresh();
  assert!(w.all(Role::Tooltip, "").is_empty());

  // Hovering shows it only after the pause.
  let hover = wrapper.on_hover.clone().expect("hover");
  hover.call(true);
  w.refresh();
  assert!(w.all(Role::Tooltip, "").is_empty());
  let start = Instant::now();
  while w.all(Role::Tooltip, "").is_empty() {
    assert!(start.elapsed() < Duration::from_secs(5), "no tooltip");
    std::thread::sleep(Duration::from_millis(20));
    run_task_messages();
    w.refresh();
  }
  assert!(start.elapsed() >= Duration::from_millis(400));
  hover.call(false);
  w.refresh();
  assert!(w.all(Role::Tooltip, "").is_empty());
}
