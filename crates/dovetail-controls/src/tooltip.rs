//! Tooltips (M15, D0021).

use std::time::Duration;

use dovetail::{
  Callback, Children, Edges, Element, ElementKind, KeyHandler, Layer, NamedKey,
  Role, Style, Task, View, use_state,
};
use dovetail_macros::component;
use dovetail_theme::theme;

/// How long the pointer rests on an element before its tooltip shows.
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(500);

/// Short help for what is inside it. It shows below, after the pointer
/// has rested there for [`TOOLTIP_DELAY`], and at once while keyboard
/// focus is inside. Escape hides it.
///
/// Assistive tools read `text` as the description of the first control
/// inside.
#[component]
pub fn Tooltip(
  #[prop(into)] text: String,
  children: Children,
) -> View {
  let t = theme();
  let shown = use_state(|| false);
  let hovering = use_state(|| false);
  let focused = use_state(|| false);
  let timer = use_state(|| None::<Task>);
  let stop = move || {
    if let Some(task) = timer.get() {
      task.cancel();
      timer.set(None);
    }
  };
  let on_hover = Callback::new(move |over: bool| {
    hovering.set(over);
    stop();
    if over {
      timer.set(Some(Task::after(
        TOOLTIP_DELAY,
        Callback::new(move |()| {
          // The tooltip may have gone while it waited (B0009).
          if timer.try_update(|t| *t = None).is_err() {
            return;
          }
          if hovering.get() {
            shown.set(true);
          }
        }),
      )));
    } else if !focused.get() {
      shown.set(false);
    }
  });
  let on_focus = Callback::new(move |inside: bool| {
    focused.set(inside);
    shown.set(inside || (hovering.get() && timer.get().is_none()));
  });
  let keys = KeyHandler::new(move |event| {
    if event.is(NamedKey::Escape) && shown.get() {
      shown.set(false);
      true
    } else {
      false
    }
  });
  let mut inside: Vec<View> = children;
  if shown.get() {
    let (background, foreground, radius) = t.tooltip_look();
    let tip: View = Element::new(ElementKind::Box)
      .style(Style {
        padding: Edges::xy(t.spacing.of(2.0), t.spacing.of(1.0)),
        background,
        radius,
        text_color: Some(foreground),
        font_size: Some(t.text.body_small.size),
        font_weight: Some(t.text.body_small.weight),
        max_width: 320.0,
        ..Style::default()
      })
      .role(Role::Tooltip)
      .children([View::text(text.clone())])
      .into();
    // A little below what it describes.
    inside.push(
      Element::new(ElementKind::Box)
        .style(Style {
          layer: Some(Layer::Below),
          padding: Edges { top: 4.0, ..Edges::default() },
          ..Style::default()
        })
        .children([tip])
        .into(),
    );
  }
  Element::new(ElementKind::Box)
    .description(text)
    .on_hover(on_hover)
    .on_focus_within(on_focus)
    .on_key(keys)
    .children(inside)
    .into()
}
