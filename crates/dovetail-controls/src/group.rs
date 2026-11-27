//! Toggle buttons and button groups (M25, D0030).

use dovetail::Animate;
use dovetail::{
  Align, Border, Callback, Color, Edges, Element, ElementKind, FocusTarget,
  Justify, KeyCode, KeyHandler, NamedKey, Role, Style, View, current_component,
  use_animated, use_state,
};
use dovetail_macros::component;
use dovetail_theme::{GroupLook, theme};

/// What a button shows: text, an icon, or both. With only an icon, the
/// `label` names it for assistive tools.
fn face(
  text: &str,
  icon: Option<&dovetail_image::Icon>,
  label: &str,
  gap: f32,
) -> View {
  match (icon, text.is_empty()) {
    (Some(icon), true) => icon
      .picture()
      .label(if label.is_empty() { text } else { label })
      .into(),
    (Some(icon), false) => Element::new(ElementKind::Row)
      .style(Style {
        gap,
        align: Align::Center,
        ..Style::default()
      })
      .children([
        icon.picture().into(),
        View::text(text.to_owned()),
      ])
      .into(),
    (None, _) => View::text(text.to_owned()),
  }
}

/// The fill and ink of a button that is `pressed` of the way in, from 0
/// to 1, mixed from the theme's colours.
fn pressed_colours(
  look: &GroupLook,
  pressed: f32,
  enabled: bool,
) -> (Color, Color) {
  if !enabled {
    let (fill, ink) = theme().disabled_look();
    return (Color { a: fill.a * pressed, ..fill }, ink);
  }
  let clear = Color { a: 0.0, ..look.pressed };
  (
    clear.animate(&look.pressed, pressed),
    look.text.animate(&look.pressed_text, pressed),
  )
}

/// A button that stays pressed until it is pressed again (M25). Pressing
/// it, or Space or Enter while it has focus, calls `on_change` with the new
/// state; the component that owns it updates `on`.
///
/// It shows `text`, an `icon`, or both. With only an icon, `label` names
/// it. Assistive tools read it as a toggle button that is pressed or not.
#[component]
pub fn ToggleButton(
  on: bool,
  #[prop(default = Callback::new(|_: bool| {}))] on_change: Callback<bool>,
  #[prop(into, default = "")] text: String,
  #[prop(default)] icon: Option<dovetail_image::Icon>,
  #[prop(into, default = "")] label: String,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  let look = t.group_look();
  let pressed = use_animated(if on { 1.0f32 } else { 0.0 }, t.motion.short());
  let (fill, ink) = pressed_colours(&look, pressed, enabled);
  let pad = t.spacing.of(1.5);
  let button = Element::new(ElementKind::Box)
    .style(Style {
      padding: if text.is_empty() {
        Edges::all(pad)
      } else {
        Edges::xy(t.spacing.of(2.5), pad)
      },
      radius: look.radius,
      background: fill,
      border: Border { width: 1.0, color: look.border },
      text_color: Some(ink),
      font_size: Some(t.text.label.size),
      font_weight: Some(t.text.label.weight),
      ..Style::default()
    })
    .role(Role::Button)
    .toggled(on)
    .children([face(
      &text,
      icon.as_ref(),
      &label,
      t.spacing.of(1.5),
    )]);
  let button = if label.is_empty() {
    button
  } else {
    button.label(label)
  };
  if enabled {
    button
      .focusable(true)
      .on_press(Callback::new(move |()| on_change.call(!on)))
      .into()
  } else {
    button.disabled(true).into()
  }
}

/// How many buttons of a [`ButtonGroup`] can be chosen (M25).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Choose {
  /// One at a time, as a text editor's alignment buttons: a segmented
  /// control. The arrow keys move the choice. The default.
  #[default]
  One,
  /// Any number, each on or off, as bold, italic and underline.
  Many,
  /// None: the buttons are plain buttons, joined.
  None,
}

/// One button of a [`ButtonGroup`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupButton {
  /// Its text. Leave it empty for an icon only.
  pub text: String,
  /// Its icon.
  pub icon: Option<dovetail_image::Icon>,
  /// Its name for assistive tools, when it has only an icon.
  pub label: String,
  /// It can be pressed. Buttons are enabled when made with
  /// [`GroupButton::text`] or [`GroupButton::icon`].
  pub enabled: bool,
}

impl GroupButton {
  /// A button with text.
  pub fn text(text: impl Into<String>) -> GroupButton {
    GroupButton {
      text: text.into(),
      enabled: true,
      ..GroupButton::default()
    }
  }

  /// A button with only an icon, named `label`.
  pub fn icon(
    icon: dovetail_image::Icon,
    label: impl Into<String>,
  ) -> GroupButton {
    GroupButton {
      icon: Some(icon),
      label: label.into(),
      enabled: true,
      ..GroupButton::default()
    }
  }

  /// The same button, enabled or not.
  pub fn enabled(
    mut self,
    enabled: bool,
  ) -> GroupButton {
    self.enabled = enabled;
    self
  }
}

/// Buttons joined side by side, or one above the other with `vertical`
/// (M25). Pressing one calls `on_press` with its place; the component that
/// owns the choice updates `chosen`, the places of the buttons shown
/// pressed.
///
/// With [`Choose::One`] it is a segmented control: one Tab stop, the
/// chosen button, and the arrow keys, Home and End move the choice. With
/// [`Choose::Many`] or [`Choose::None`] it is still one Tab stop; the arrow
/// keys move focus, and Space or Enter presses. `label` names the group.
#[component]
pub fn ButtonGroup(
  buttons: Vec<GroupButton>,
  #[prop(default)] chosen: Vec<usize>,
  #[prop(default = Callback::new(|_: usize| {}))] on_press: Callback<usize>,
  #[prop(default)] choose: Choose,
  #[prop(default)] vertical: bool,
  #[prop(into, default = "")] label: String,
) -> View {
  let t = theme();
  let look = t.group_look();
  let count = buttons.len();
  let owner = current_component();
  let target = move |i: usize| {
    FocusTarget::named(&format!("dovetail.group.{owner:?}.{i}"))
  };
  // The button the keyboard is on, for groups whose choice is not the
  // Tab stop.
  let at = use_state(|| None::<usize>);
  let enabled_at: Vec<bool> = buttons.iter().map(|b| b.enabled).collect();
  let tab_stop = match choose {
    Choose::One => chosen.first().copied(),
    _ => at.get(),
  }
  .filter(|&i| i < count && enabled_at[i])
  .or_else(|| enabled_at.iter().position(|&e| e));
  let pad = t.spacing.of(1.5);
  let row: Vec<View> = buttons
    .iter()
    .enumerate()
    .map(|(i, b)| {
      let on = chosen.contains(&i);
      let view = View::component::<GroupSegment>(GroupSegmentProps {
        button: b.clone(),
        on,
        stateful: choose != Choose::None,
        radio: choose == Choose::One,
        target: target(i),
        tab_stop: tab_stop == Some(i),
        radius: (look.radius - 2.0).max(0.0),
        pad,
        on_press: {
          let on_press = on_press.clone();
          Callback::new(move |()| {
            on_press.call(i);
            // The pressed button becomes the Tab stop. (Set on press, not
            // on focus: a change between press and release would lose the
            // click.)
            at.set(Some(i));
            target(i).focus();
          })
        },
      });
      view.keyed(i as u64)
    })
    .collect();
  let keys = KeyHandler::new(move |event| {
    if count == 0 {
      return false;
    }
    let from = match choose {
      Choose::One => chosen.first().copied(),
      _ => at.get(),
    }
    .unwrap_or(0);
    let forward = if vertical {
      NamedKey::ArrowDown
    } else {
      NamedKey::ArrowRight
    };
    let back = if vertical {
      NamedKey::ArrowUp
    } else {
      NamedKey::ArrowLeft
    };
    let step = |dir: isize| {
      (1..=count)
        .map(|k| {
          (from as isize + dir * k as isize).rem_euclid(count as isize) as usize
        })
        .find(|&i| enabled_at[i])
    };
    let next = match &event.key {
      KeyCode::Named(k) if *k == forward => step(1),
      KeyCode::Named(k) if *k == back => step(-1),
      KeyCode::Named(NamedKey::Home) => enabled_at.iter().position(|&e| e),
      KeyCode::Named(NamedKey::End) => enabled_at.iter().rposition(|&e| e),
      _ => return false,
    };
    if let Some(i) = next {
      if choose == Choose::One {
        on_press.call(i);
      }
      at.set(Some(i));
      target(i).focus();
    }
    true
  });
  Element::new(if vertical {
    ElementKind::Column
  } else {
    ElementKind::Row
  })
  .style(Style {
    padding: Edges::all(2.0),
    gap: 2.0,
    align: Align::Stretch,
    background: look.background,
    border: Border { width: 1.0, color: look.border },
    radius: look.radius,
    ..Style::default()
  })
  .role(if choose == Choose::One {
    Role::RadioGroup
  } else {
    Role::Group
  })
  .label(label)
  .on_key(keys)
  .children(row)
  .into()
}

/// One button of a group, a component of its own so its press animates.
#[component]
fn GroupSegment(
  button: GroupButton,
  on: bool,
  stateful: bool,
  radio: bool,
  target: FocusTarget,
  tab_stop: bool,
  radius: f32,
  pad: f32,
  on_press: Callback<()>,
) -> View {
  let t = theme();
  let look = t.group_look();
  let pressed = use_animated(if on { 1.0f32 } else { 0.0 }, t.motion.short());
  let (fill, ink) = pressed_colours(&look, pressed, button.enabled);
  let mut e = Element::new(ElementKind::Row)
    .style(Style {
      padding: if button.text.is_empty() {
        Edges::all(pad)
      } else {
        Edges::xy(t.spacing.of(2.5), pad)
      },
      radius,
      align: Align::Center,
      justify: Justify::Center,
      background: fill,
      text_color: Some(ink),
      font_size: Some(t.text.label.size),
      font_weight: Some(t.text.label.weight),
      ..Style::default()
    })
    .role(if radio {
      Role::RadioButton
    } else {
      Role::Button
    })
    .focus_target(target)
    .children([face(
      &button.text,
      button.icon.as_ref(),
      &button.label,
      t.spacing.of(1.5),
    )]);
  if !button.label.is_empty() {
    e = e.label(button.label.clone());
  }
  if stateful {
    e = e.toggled(on);
  }
  if button.enabled {
    e.focusable(tab_stop).on_press(on_press).into()
  } else {
    e.disabled(true).into()
  }
}
