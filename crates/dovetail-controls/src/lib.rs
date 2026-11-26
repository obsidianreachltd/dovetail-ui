//! Buttons, toggles, check boxes, radio buttons, drop-downs, number
//! fields, sliders, tooltips, tabs, text, text fields and layout
//! containers for Dovetail UI.
//!
//! These are written with `#[component]`, the same way as any third-party
//! component, and are built on elements (`dovetail_core::Element`). Their
//! colours, text and shapes come from the nearest theme (M14,
//! `dovetail_theme::theme`).

// The macros refer to the core as `::dovetail`.
extern crate dovetail_core as dovetail;

use dovetail::Animate;
use dovetail::{
  Align, Callback, Children, Color, Edges, Element, ElementKind, Role, Size,
  Style, View, use_animated,
};
use dovetail_macros::component;
use dovetail_theme::theme;

mod canvas;
mod choice;
mod dialog;
mod media;
mod pieces;
mod select;
mod text_field;
mod tooltip;
mod values;

pub use canvas::{Camera, SelectionBox};
pub use choice::{CheckBox, RadioGroup};
pub use dialog::{Dialog, DialogButton, ProgressBar};
pub use media::{
  Icon, IconButton, Image, ImageSource, ImageState, image_decodes,
};
pub use pieces::{Collapsible, Link, Separator, StatusBar, Tabs};
pub use select::Select;
pub use text_field::TextField;
pub use tooltip::{TOOLTIP_DELAY, Tooltip};
pub use values::{NumberField, Slider};

/// `text` with `icon` before it, if any, centred in a row.
pub(crate) fn with_icon(
  icon: Option<dovetail_image::Icon>,
  text: String,
  gap: f32,
) -> View {
  match icon {
    None => View::text(text),
    Some(icon) => Element::new(ElementKind::Row)
      .style(Style {
        gap,
        align: Align::Center,
        ..Style::default()
      })
      .children([icon.picture().into(), View::text(text)])
      .into(),
  }
}

fn container(
  kind: ElementKind,
  gap: f32,
  children: Children,
) -> View {
  Element::new(kind)
    .style(Style { gap, ..Style::default() })
    .children(children)
    .into()
}

/// Places its children one below the other, with `gap` logical pixels
/// between them (8 by default).
#[component]
pub fn Column(
  children: Children,
  #[prop(default = 8.0)] gap: f32,
) -> View {
  container(ElementKind::Column, gap, children)
}

/// Places its children side by side, with `gap` logical pixels between them
/// (8 by default).
#[component]
pub fn Row(
  children: Children,
  #[prop(default = 8.0)] gap: f32,
) -> View {
  container(ElementKind::Row, gap, children)
}

/// Places its children on top of each other, the last one in front.
#[component]
pub fn Stack(children: Children) -> View {
  container(ElementKind::Stack, 0.0, children)
}

/// Shows one line of text. Without `size` or `color` it takes them from
/// its parents.
#[component]
pub fn Text(
  #[prop(into)] value: String,
  #[prop(default)] size: Option<f32>,
  #[prop(default)] color: Option<Color>,
) -> View {
  if size.is_none() && color.is_none() {
    return View::text(value);
  }
  Element::new(ElementKind::Box)
    .style(Style {
      font_size: size,
      text_color: color,
      ..Style::default()
    })
    .children([View::text(value)])
    .into()
}

/// A button that calls `on_click` when it is clicked. With `enabled` set
/// to false it is greyed out, cannot be pressed or take focus, and
/// assistive tools say it is unavailable.
///
/// An `icon` shows before the text (M17), as decoration: the text names
/// the button. For a button with only an icon, use [`IconButton`].
#[component]
pub fn Button(
  #[prop(into)] text: String,
  #[prop(default = Callback::new(|_: ()| {}))] on_click: Callback<()>,
  #[prop(default = true)] enabled: bool,
  // An icon before the text (M17), as decoration: the text names the
  // button.
  #[prop(default)] icon: Option<dovetail_image::Icon>,
) -> View {
  let t = theme();
  let (background, foreground, radius, padding) = t.button_look();
  let (off_background, off_foreground) = t.disabled_look();
  let button = Element::new(ElementKind::Box)
    .style(Style {
      padding,
      background: if enabled { background } else { off_background },
      radius,
      text_color: Some(if enabled { foreground } else { off_foreground }),
      font_size: Some(t.text.label.size),
      font_weight: Some(t.text.label.weight),
      ..Style::default()
    })
    .role(Role::Button)
    .children([with_icon(icon, text, t.spacing.of(1.5))]);
  if enabled {
    button.focusable(true).on_press(on_click).into()
  } else {
    button.disabled(true).into()
  }
}

/// A switch that is on or off. Clicking it calls `on_change` with the new
/// value; the component that owns the value updates `on`.
///
/// The knob slides and the track's colour fades over the theme's short
/// [`Motion`](dovetail_theme::Motion) time, unless less motion is set
/// (M18).
#[component]
pub fn Toggle(
  on: bool,
  #[prop(default = Callback::new(|_: bool| {}))] on_change: Callback<bool>,
  #[prop(into, default = "")] label: String,
) -> View {
  let t = theme();
  let (track_on, track_off, knob_colour) = t.toggle_look();
  // How far on it is, from 0 to 1. The colours are mixed from it, so a
  // theme that fades reaches the track at once.
  let shown = use_animated(if on { 1.0f32 } else { 0.0 }, t.motion.short());
  // 36 wide, with 2 of padding each side and a 16 knob: it moves 16.
  let travel = 16.0 * shown;
  let background = track_off.animate(&track_on, shown);
  let knob: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(16.0),
      height: Size::Fixed(16.0),
      radius: 8.0,
      background: knob_colour,
      offset: (travel, 0.0),
      ..Style::default()
    })
    .into();
  let track: View = Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fixed(36.0),
      height: Size::Fixed(20.0),
      padding: Edges::all(2.0),
      radius: 10.0,
      background,
      align: Align::Center,
      ..Style::default()
    })
    .children([knob])
    .into();
  let mut children = vec![track];
  if !label.is_empty() {
    children.push(View::text(label));
  }
  Element::new(ElementKind::Row)
    .style(Style {
      gap: 8.0,
      align: Align::Center,
      ..Style::default()
    })
    .role(Role::Switch)
    .toggled(on)
    .focusable(true)
    .on_press(Callback::new(move |()| on_change.call(!on)))
    .children(children)
    .into()
}
