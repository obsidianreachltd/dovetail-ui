//! Number fields and sliders (M15).

use dovetail::{
  AccessAction, Align, Callback, Edges, Element, ElementKind, KeyCode,
  KeyHandler, NamedKey, PointerHandler, PointerKind, Role, Size, Style,
  ValueRange, View, use_state,
};
use dovetail_macros::{component, ui};
use dovetail_theme::theme;

use crate::text_field::TextField;

/// `value` moved by `steps` steps of `range`, kept inside it, and on a
/// whole step from `min` when the range has steps.
pub(crate) fn stepped(
  value: f32,
  range: ValueRange,
  steps: f32,
) -> f32 {
  let step = if range.step > 0.0 {
    range.step
  } else {
    (range.max - range.min) / 100.0
  };
  snap(value + steps * step, range)
}

/// `value` kept inside `range`, on a whole step from `min`.
pub(crate) fn snap(
  value: f32,
  range: ValueRange,
) -> f32 {
  let v = value.clamp(range.min, range.max);
  if range.step > 0.0 {
    let n = ((v - range.min) / range.step).round();
    (range.min + n * range.step).clamp(range.min, range.max)
  } else {
    v
  }
}

/// The value after a key, for a control that steps through `range`, or
/// `None` when the key is not one of its own: the arrows move one step,
/// Page Up and Page Down ten, and Home and End go to the ends.
pub(crate) fn value_after_key(
  value: f32,
  range: ValueRange,
  key: &KeyCode,
) -> Option<f32> {
  Some(match key {
    KeyCode::Named(NamedKey::ArrowUp | NamedKey::ArrowRight) => {
      stepped(value, range, 1.0)
    },
    KeyCode::Named(NamedKey::ArrowDown | NamedKey::ArrowLeft) => {
      stepped(value, range, -1.0)
    },
    KeyCode::Named(NamedKey::PageUp) => stepped(value, range, 10.0),
    KeyCode::Named(NamedKey::PageDown) => stepped(value, range, -10.0),
    KeyCode::Named(NamedKey::Home) => range.min,
    KeyCode::Named(NamedKey::End) => range.max,
    _ => return None,
  })
}

/// The value an assistive tool asked for.
fn value_after_access(
  value: f32,
  range: ValueRange,
  action: &AccessAction,
) -> Option<f32> {
  match action {
    AccessAction::Increment => Some(stepped(value, range, 1.0)),
    AccessAction::Decrement => Some(stepped(value, range, -1.0)),
    AccessAction::SetNumber(n) => Some(snap(*n, range)),
    AccessAction::SetValue(text) => {
      text.trim().parse().ok().map(|n| snap(n, range))
    },
    _ => None,
  }
}

/// Calls `on_change` only when the value is new.
fn change(
  on_change: &Callback<f32>,
  old: f32,
  new: f32,
) -> bool {
  if new != old {
    on_change.call(new);
  }
  true
}

/// `value` as text, with `decimals` digits after the point.
fn format(
  value: f32,
  decimals: usize,
) -> String {
  format!("{value:.decimals$}")
}

/// A field for a number from `min` to `max`. Typing a number in range
/// calls `on_change`; leaving the field shows the value again, so text
/// that is not a number goes away.
///
/// The arrow keys move the value by `step`, Page Up and Page Down by ten
/// steps, and the buttons on the right by one. While the field has focus,
/// the mouse wheel moves it too. `decimals` is how many digits it shows
/// after the point.
#[component]
pub fn NumberField(
  value: f32,
  #[prop(default = Callback::new(|_: f32| {}))] on_change: Callback<f32>,
  #[prop(default = f32::MIN)] min: f32,
  #[prop(default = f32::MAX)] max: f32,
  #[prop(default = 1.0)] step: f32,
  #[prop(default)] decimals: usize,
  #[prop(default = 120.0)] width: f32,
  #[prop(into, default = "")] label: String,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  let range = ValueRange { min, max, step };
  let field_height = crate::text_field::field_height(t.text.body.size);
  let text = use_state(|| format(value, decimals));
  // The value last shown, so a new value from the owner replaces the text.
  let shown = use_state(|| value);
  let focused = use_state(|| false);
  if shown.get() != value {
    shown.set(value);
    text.set(format(value, decimals));
  }

  let typed = {
    let on_change = on_change.clone();
    Callback::new(move |s: String| {
      text.set(s.clone());
      if let Ok(n) = s.trim().parse::<f32>()
        && (min..=max).contains(&n)
      {
        shown.set(n);
        change(&on_change, value, n);
      }
    })
  };
  let keys = {
    let on_change = on_change.clone();
    KeyHandler::new(move |event| {
      enabled
        && value_after_key(value, range, &event.key)
          .filter(|_| {
            !matches!(
              event.key,
              KeyCode::Named(
                NamedKey::Home
                  | NamedKey::End
                  | NamedKey::ArrowLeft
                  | NamedKey::ArrowRight
              )
            )
          })
          .is_some_and(|new| change(&on_change, value, new))
    })
  };
  let button = |up: bool| -> View {
    let on_change = on_change.clone();
    let b = Element::new(ElementKind::Row)
      .style(Style {
        width: Size::Fixed(18.0),
        // The two share the field's height.
        height: Size::Fixed(((field_height - 1.0) / 2.0).floor()),
        align: Align::Center,
        justify: dovetail::Justify::Center,
        radius: 2.0,
        background: t.colors.surface_container_high,
        font_size: Some(8.0),
        text_color: Some(if enabled {
          t.colors.on_surface_variant
        } else {
          t.disabled_look().1
        }),
        ..Style::default()
      })
      .role(Role::Button)
      .label(if up { "Increase" } else { "Decrease" })
      .children([if up {
        dovetail_image::icons::chevron_up()
      } else {
        dovetail_image::icons::chevron_down()
      }
      .picture()
      .size(10.0)
      .into()]);
    if enabled {
      b.on_press(Callback::new(move |()| {
        change(
          &on_change,
          value,
          stepped(value, range, if up { 1.0 } else { -1.0 }),
        );
      }))
      .into()
    } else {
      b.disabled(true).into()
    }
  };
  let buttons: View = Element::new(ElementKind::Column)
    .style(Style { gap: 1.0, ..Style::default() })
    .children([button(true), button(false)])
    .into();
  let field_width = (width - 20.0).max(40.0);
  let field_label = label.clone();
  let field = ui! {
    TextField {
      value: text.get(),
      on_change: move |s| typed.call(s),
      width: field_width,
      label: field_label,
    }
  };
  let wheel = {
    let on_change = on_change.clone();
    Callback::new(move |w: dovetail::WheelEvent| {
      if focused.get() && enabled && w.dy != 0.0 {
        let steps = if w.dy < 0.0 { 1.0 } else { -1.0 };
        change(&on_change, value, stepped(value, range, steps));
      }
    })
  };
  let access = Callback::new(move |action: AccessAction| {
    if let Some(new) = value_after_access(value, range, &action) {
      change(&on_change, value, new);
    }
  });
  let row = Element::new(ElementKind::Row)
    .style(Style {
      gap: 2.0,
      align: Align::Stretch,
      ..Style::default()
    })
    .role(Role::SpinButton)
    .label(label)
    .value_in(value, range)
    .on_focus_within(Callback::new(move |f| {
      focused.set(f);
      if !f {
        // Leaving the field shows the value, not what was typed.
        text.set(format(shown.get(), decimals));
      }
    }))
    .children([field, buttons]);
  if enabled {
    row
      .on_key(keys)
      .on_wheel(wheel)
      .on_access(access)
      .into()
  } else {
    row.disabled(true).into()
  }
}

/// The size of a slider's thumb.
const THUMB: f32 = 16.0;

/// A value from `min` to `max`, set by dragging the thumb along a track
/// `width` pixels long, or by clicking the track.
///
/// With focus, the arrow keys move it by `step`, Page Up and Page Down by
/// ten steps, and Home and End to the ends. A step of 0 allows any value,
/// and the keys move it by a hundredth of the range.
#[component]
pub fn Slider(
  value: f32,
  #[prop(default = Callback::new(|_: f32| {}))] on_change: Callback<f32>,
  #[prop(default = 0.0)] min: f32,
  #[prop(default = 100.0)] max: f32,
  #[prop(default = 1.0)] step: f32,
  #[prop(default = 200.0)] width: f32,
  #[prop(into, default = "")] label: String,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  let range = ValueRange { min, max, step };
  let (mut active, mut track, mut thumb) = t.slider_look();
  if !enabled {
    let (off, ink) = t.disabled_look();
    (active, track, thumb) = (ink, off, ink);
  }
  let span = (max - min).max(f32::EPSILON);
  let part = ((value - min) / span).clamp(0.0, 1.0);
  let room = (width - THUMB).max(0.0);
  let at = part * room;
  let bar = |length: f32, colour| -> View {
    Element::new(ElementKind::Box)
      .style(Style {
        width: Size::Fixed(length),
        height: Size::Fixed(4.0),
        radius: 2.0,
        background: colour,
        ..Style::default()
      })
      .into()
  };
  let knob: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(THUMB),
      height: Size::Fixed(THUMB),
      radius: THUMB / 2.0,
      background: thumb,
      ..Style::default()
    })
    .into();
  let pointer = {
    let on_change = on_change.clone();
    PointerHandler::new(move |event| {
      if matches!(event.kind, PointerKind::Down | PointerKind::Move) {
        let part = ((event.x - THUMB / 2.0) / room.max(1.0)).clamp(0.0, 1.0);
        change(&on_change, value, snap(min + part * span, range));
      }
    })
  };
  let keys = {
    let on_change = on_change.clone();
    KeyHandler::new(move |event| {
      value_after_key(value, range, &event.key)
        .is_some_and(|new| change(&on_change, value, new))
    })
  };
  let access = Callback::new(move |action: AccessAction| {
    if let Some(new) = value_after_access(value, range, &action) {
      change(&on_change, value, new);
    }
  });
  let slider = Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fixed(width),
      height: Size::Fixed(THUMB + 4.0),
      padding: Edges::xy(0.0, 2.0),
      align: Align::Center,
      ..Style::default()
    })
    .role(Role::Slider)
    .label(label)
    .value_in(value, range)
    .children([
      bar(at, active),
      knob,
      bar(room - at, track),
    ]);
  if enabled {
    slider
      .focusable(true)
      .on_pointer(pointer)
      .on_key(keys)
      .on_access(access)
      .into()
  } else {
    slider.disabled(true).into()
  }
}
