//! Check boxes and radio buttons (M15).

use dovetail::{
  Align, Border, Callback, Color, Element, ElementKind, FocusTarget, Justify,
  KeyCode, KeyHandler, NamedKey, Role, Size, Style, View, current_component,
};
use dovetail_macros::component;
use dovetail_theme::{ThemeData, theme};

/// The size of a check box's box and a radio button's circle.
const MARK: f32 = 16.0;

/// A box and its label, side by side.
fn with_label(
  mark: View,
  label: &str,
  ink: Color,
) -> Vec<View> {
  let mut children = vec![mark];
  if !label.is_empty() {
    children.push(
      Element::new(ElementKind::Box)
        .style(Style {
          text_color: Some(ink),
          ..Style::default()
        })
        .children([View::text(label.to_owned())])
        .into(),
    );
  }
  children
}

/// The colour of a label: greyed out when the control cannot be used.
fn label_ink(
  t: &ThemeData,
  enabled: bool,
) -> Color {
  if enabled {
    t.colors.on_surface
  } else {
    t.disabled_look().1
  }
}

/// A box that is ticked or not. Clicking it, or Space while it has focus,
/// calls `on_change` with the new value; the component that owns the value
/// updates `checked`.
///
/// With `mixed`, it shows a dash: what it stands for is partly on, as a
/// "select all" box over a list that is partly chosen. Clicking a mixed box
/// turns it on.
#[component]
pub fn CheckBox(
  checked: bool,
  #[prop(default = Callback::new(|_: bool| {}))] on_change: Callback<bool>,
  #[prop(into, default = "")] label: String,
  #[prop(default)] mixed: bool,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  let (on_fill, mark_ink, border) = t.check_look();
  let (off_fill, off_ink) = t.disabled_look();
  let filled = checked || mixed;
  let fill = match (enabled, filled) {
    (true, true) => on_fill,
    (false, true) => off_ink,
    (_, false) => Color::TRANSPARENT,
  };
  let mark: View = if mixed {
    Element::new(ElementKind::Box)
      .style(Style {
        width: Size::Fixed(8.0),
        height: Size::Fixed(2.0),
        background: mark_ink,
        ..Style::default()
      })
      .into()
  } else if checked {
    Element::new(ElementKind::Box)
      .style(Style {
        text_color: Some(if enabled { mark_ink } else { off_fill }),
        ..Style::default()
      })
      .children([dovetail_image::icons::check()
        .picture()
        .size(14.0)
        .into()])
      .into()
  } else {
    View::Empty
  };
  let square: View = Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fixed(MARK),
      height: Size::Fixed(MARK),
      radius: 3.0,
      background: fill,
      border: Border {
        width: if filled { 0.0 } else { 1.5 },
        color: if enabled { border } else { off_ink },
      },
      align: Align::Center,
      justify: Justify::Center,
      ..Style::default()
    })
    .children([mark])
    .into();
  let row = Element::new(ElementKind::Row)
    .style(Style {
      gap: t.spacing.of(2.0),
      align: Align::Center,
      ..Style::default()
    })
    .role(Role::CheckBox)
    .label(label.clone())
    .toggled(checked)
    .mixed(mixed)
    .children(with_label(square, &label, label_ink(&t, enabled)));
  if enabled {
    row
      .focusable(true)
      .on_press(Callback::new(move |()| on_change.call(mixed || !checked)))
      .into()
  } else {
    row.disabled(true).into()
  }
}

/// A set of choices of which one is chosen, shown as round buttons one
/// below the other (or side by side, with `across`). Clicking one calls
/// `on_change` with its position; the component that owns the choice
/// updates `selected`.
///
/// Tab moves to the chosen button and on out of the group; the arrow keys
/// move the choice, and focus with it, as on every system. `label` names
/// the group for assistive tools.
#[component]
pub fn RadioGroup(
  options: Vec<String>,
  #[prop(default)] selected: Option<usize>,
  #[prop(default = Callback::new(|_: usize| {}))] on_change: Callback<usize>,
  #[prop(into, default = "")] label: String,
  #[prop(default = true)] enabled: bool,
  #[prop(default)] across: bool,
) -> View {
  let t = theme();
  let (on_fill, _, border) = t.check_look();
  let (_, off_ink) = t.disabled_look();
  let count = options.len();
  // Each button can be given focus by its place, for the arrow keys.
  let owner = current_component();
  let target = move |i: usize| {
    FocusTarget::named(&format!("dovetail.radio.{owner:?}.{i}"))
  };
  // Tab reaches the chosen button, or the first when none is chosen.
  let tab_stop = selected.unwrap_or(0);
  let buttons: Vec<View> = options
    .iter()
    .enumerate()
    .map(|(i, text)| {
      let on = selected == Some(i);
      let dot: View = if on {
        Element::new(ElementKind::Box)
          .style(Style {
            width: Size::Fixed(8.0),
            height: Size::Fixed(8.0),
            radius: 4.0,
            background: if enabled { on_fill } else { off_ink },
            ..Style::default()
          })
          .into()
      } else {
        View::Empty
      };
      let ring = if !enabled {
        off_ink
      } else if on {
        on_fill
      } else {
        border
      };
      let circle: View = Element::new(ElementKind::Row)
        .style(Style {
          width: Size::Fixed(MARK),
          height: Size::Fixed(MARK),
          radius: MARK / 2.0,
          border: Border { width: 1.5, color: ring },
          align: Align::Center,
          justify: Justify::Center,
          ..Style::default()
        })
        .children([dot])
        .into();
      let button = Element::new(ElementKind::Row)
        .style(Style {
          gap: t.spacing.of(2.0),
          align: Align::Center,
          ..Style::default()
        })
        .role(Role::RadioButton)
        .toggled(on)
        .focus_target(target(i))
        .children(with_label(circle, text, label_ink(&t, enabled)));
      if enabled {
        let on_change = on_change.clone();
        button
          .focusable(i == tab_stop)
          .on_press(Callback::new(move |()| {
            on_change.call(i);
            // The chosen button is the one that takes focus.
            target(i).focus();
          }))
          .into()
      } else {
        button.disabled(true).into()
      }
    })
    .collect();
  let keys = KeyHandler::new(move |event| {
    if !enabled || count == 0 {
      return false;
    }
    let at = selected.unwrap_or(0);
    let next = match &event.key {
      KeyCode::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => {
        (at + 1) % count
      },
      KeyCode::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => {
        (at + count - 1) % count
      },
      KeyCode::Named(NamedKey::Home) => 0,
      KeyCode::Named(NamedKey::End) => count - 1,
      _ => return false,
    };
    on_change.call(next);
    target(next).focus();
    true
  });
  Element::new(if across {
    ElementKind::Row
  } else {
    ElementKind::Column
  })
  .style(Style {
    gap: t.spacing.of(if across { 4.0 } else { 2.0 }),
    ..Style::default()
  })
  .role(Role::RadioGroup)
  .label(label)
  .on_key(keys)
  .children(buttons)
  .into()
}
