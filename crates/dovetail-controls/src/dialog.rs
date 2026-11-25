//! A progress bar and a dialog (M11), in the theme's colours (M14).

use dovetail::{
  Align, Border, Callback, Color, Element, ElementKind, Justify, KeyCode,
  KeyHandler, Layer, NamedKey, Role, Size, Style, View,
};
use dovetail_macros::component;
use dovetail_theme::{ThemeData, theme};

/// A bar that fills as work gets done: `value` is the part done, from 0
/// to 1. Without a value the bar is empty, for work whose length is not
/// known yet. `label` names it for assistive tools.
#[component]
pub fn ProgressBar(
  #[prop(default)] value: Option<f32>,
  #[prop(default = 240.0)] width: f32,
  #[prop(into, default = "Progress")] label: String,
) -> View {
  let t = theme();
  let part = value.unwrap_or(0.0).clamp(0.0, 1.0);
  let fill: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(width * part),
      height: Size::Fill,
      radius: 4.0,
      background: t.colors.primary,
      ..Style::default()
    })
    .into();
  let bar = Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fixed(width),
      height: Size::Fixed(8.0),
      radius: 4.0,
      background: t.colors.surface_container_highest,
      ..Style::default()
    })
    .role(Role::ProgressBar)
    .label(label)
    .children([fill]);
  match value {
    Some(v) => bar.value(v).into(),
    None => bar.into(),
  }
}

/// A button in a [`Dialog`].
#[derive(Clone, Debug, PartialEq)]
pub struct DialogButton {
  /// What the button says.
  pub text: String,
  /// What it does.
  pub on_click: Callback<()>,
}

impl DialogButton {
  /// A button that says `text` and calls `on_click`.
  pub fn new(
    text: impl Into<String>,
    on_click: impl Fn() + 'static,
  ) -> DialogButton {
    DialogButton {
      text: text.into(),
      on_click: Callback::new(move |()| on_click()),
    }
  }
}

/// A box that asks a question above the rest of the window. Clicks and
/// keys do not reach what is behind it. The first button takes focus, so
/// Enter picks it, and Escape calls `on_cancel`.
///
/// Show it only while the question is open: the application keeps whether
/// it is open in its own state.
///
/// An `icon`, such as `icons::warning()`, shows before the title (M17), in
/// the title's colour, as decoration: the title says what it means.
#[component]
pub fn Dialog(
  #[prop(into)] title: String,
  #[prop(into, default = "")] message: String,
  buttons: Vec<DialogButton>,
  on_cancel: Callback<()>,
  #[prop(default)] content: Option<View>,
  #[prop(default)] icon: Option<dovetail_image::Icon>,
) -> View {
  let t = theme();
  let (surface, scrim, radius) = t.dialog_look();
  let backdrop: View = Element::new(ElementKind::Box)
    .style(Style {
      layer: Some(Layer::Fill),
      background: scrim,
      ..Style::default()
    })
    // A click outside the box does nothing.
    .on_press(Callback::new(|()| {}))
    .into();
  let heading: View = Element::new(ElementKind::Box)
    .style(Style {
      font_size: Some(t.text.title.size),
      font_weight: Some(t.text.title.weight),
      ..Style::default()
    })
    .role(Role::Heading)
    .children([crate::with_icon(
      icon,
      title.clone(),
      t.spacing.of(2.0),
    )])
    .into();
  let mut children = vec![heading];
  if !message.is_empty() {
    children.push(
      Element::new(ElementKind::Box)
        .role(Role::Label)
        .children([View::text(message)])
        .into(),
    );
  }
  if let Some(content) = content {
    children.push(content);
  }
  let row: Vec<View> = buttons
    .into_iter()
    .enumerate()
    .map(|(i, b)| button(&t, b, i == 0))
    .collect();
  children.push(
    Element::new(ElementKind::Row)
      .style(Style {
        gap: t.spacing.of(2.0),
        justify: Justify::End,
        width: Size::Fill,
        ..Style::default()
      })
      .children(row)
      .into(),
  );
  let keys = KeyHandler::new(move |event| {
    if event.key == KeyCode::Named(NamedKey::Escape) {
      on_cancel.call(());
      true
    } else {
      false
    }
  });
  let panel: View = Element::new(ElementKind::Column)
    .style(Style {
      layer: Some(Layer::TopCentre(120.0)),
      modal: true,
      min_width: 320.0,
      max_width: 520.0,
      padding: dovetail::Edges::all(t.spacing.of(4.5)),
      gap: t.spacing.of(3.0),
      align: Align::Stretch,
      background: surface,
      border: Border {
        width: 1.0,
        color: t.colors.outline_variant,
      },
      radius,
      text_color: Some(t.colors.on_surface),
      font_size: Some(t.text.body.size),
      font_weight: Some(t.text.body.weight),
      ..Style::default()
    })
    .role(Role::Dialog)
    .label(title)
    .on_key(keys)
    // Clicks inside the box do not reach the backdrop.
    .on_press(Callback::new(|()| {}))
    .children(children)
    .into();
  View::fragment([backdrop, panel])
}

/// A dialog button. The first one is filled and takes focus; the others
/// are outlined.
fn button(
  t: &ThemeData,
  b: DialogButton,
  first: bool,
) -> View {
  let (fill, ink, radius, padding) = t.button_look();
  let c = &t.colors;
  Element::new(ElementKind::Box)
    .style(Style {
      padding,
      background: if first { fill } else { Color::TRANSPARENT },
      border: Border {
        width: 1.0,
        color: if first { fill } else { c.outline },
      },
      radius,
      text_color: Some(if first { ink } else { c.on_surface }),
      font_size: Some(t.text.label.size),
      font_weight: Some(t.text.label.weight),
      ..Style::default()
    })
    .role(Role::Button)
    .focusable(true)
    .autofocus(first)
    .on_press(b.on_click)
    .children([View::text(b.text)])
    .into()
}
