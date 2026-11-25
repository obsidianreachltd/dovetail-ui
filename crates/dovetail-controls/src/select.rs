//! A drop-down list to choose from (M15).

use dovetail::{
  Align, Border, Callback, Color, Edges, Element, ElementKind, Justify,
  KeyCode, KeyHandler, Layer, NamedKey, Role, Signal, Size, Style, View,
  use_state,
};
use dovetail_macros::component;
use dovetail_theme::theme;

/// The next option after `from` whose text starts with `letter`, ignoring
/// case, going round to the start: typing a letter again moves on.
fn by_letter(
  options: &[String],
  from: Option<usize>,
  letter: &str,
) -> Option<usize> {
  let letter = letter.to_lowercase();
  let n = options.len();
  let start = from.map_or(0, |f| f + 1);
  (0..n)
    .map(|k| (start + k) % n)
    .find(|&i| options[i].to_lowercase().starts_with(&letter))
}

/// A box that shows the chosen option and opens a list of all of them.
///
/// Clicking it, or Enter, Space or Alt+Down while it has focus, opens the
/// list below it. In the list, the arrow keys move the highlight, Enter
/// chooses, Escape closes, and typing a letter moves to the next option
/// that starts with it. While the list is closed, the arrow keys and
/// letters change the choice at once, as on Windows.
///
/// Choosing calls `on_change` with the option's position; the component
/// that owns the choice updates `selected`.
#[component]
pub fn Select(
  options: Vec<String>,
  #[prop(default)] selected: Option<usize>,
  #[prop(default = Callback::new(|_: usize| {}))] on_change: Callback<usize>,
  #[prop(into, default = "")] label: String,
  #[prop(into, default = "")] placeholder: String,
  #[prop(default = 200.0)] width: f32,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  let open = use_state(|| false);
  let highlight = use_state(|| None::<usize>);
  let is_open = open.get() && enabled;
  let count = options.len();

  let choose = {
    let on_change = on_change.clone();
    move |i: usize, open: Signal<bool>| {
      open.set(false);
      if Some(i) != selected {
        on_change.call(i);
      }
    }
  };
  let keys = {
    let options = options.clone();
    let choose = choose.clone();
    let on_change = on_change.clone();
    KeyHandler::new(move |event| {
      if !enabled || count == 0 {
        return false;
      }
      let alt = event.modifiers.alt;
      if !open.get() {
        // Closed: open, or change the choice at once.
        let next = match &event.key {
          KeyCode::Named(NamedKey::Enter | NamedKey::Space) => None,
          KeyCode::Named(NamedKey::ArrowDown) if alt => None,
          KeyCode::Named(NamedKey::ArrowDown) => {
            Some(selected.map_or(0, |s| (s + 1).min(count - 1)))
          },
          KeyCode::Named(NamedKey::ArrowUp) => {
            Some(selected.map_or(0, |s| s.saturating_sub(1)))
          },
          KeyCode::Named(NamedKey::Home) => Some(0),
          KeyCode::Named(NamedKey::End) => Some(count - 1),
          KeyCode::Character(c) if !event.modifiers.command() && !alt => {
            match by_letter(&options, selected, c) {
              Some(i) => Some(i),
              None => return true,
            }
          },
          _ => return false,
        };
        match next {
          Some(i) => {
            if Some(i) != selected {
              on_change.call(i);
            }
          },
          None => {
            highlight.set(selected.or(Some(0)));
            open.set(true);
          },
        }
        return true;
      }
      // Open: move the highlight, choose or close.
      let at = highlight.get();
      match &event.key {
        KeyCode::Named(NamedKey::ArrowDown) => {
          highlight.set(Some(at.map_or(0, |h| (h + 1).min(count - 1))))
        },
        KeyCode::Named(NamedKey::ArrowUp) => {
          highlight.set(Some(at.map_or(0, |h| h.saturating_sub(1))))
        },
        KeyCode::Named(NamedKey::Home) => highlight.set(Some(0)),
        KeyCode::Named(NamedKey::End) => highlight.set(Some(count - 1)),
        KeyCode::Named(NamedKey::Enter | NamedKey::Space) => match at {
          Some(i) => choose(i, open),
          None => open.set(false),
        },
        KeyCode::Named(NamedKey::Escape | NamedKey::Tab) => {
          open.set(false);
          // Tab still moves focus on.
          return event.is(NamedKey::Escape);
        },
        KeyCode::Character(c) => {
          if let Some(i) = by_letter(&options, at, c) {
            highlight.set(Some(i));
          }
        },
        _ => return false,
      }
      true
    })
  };

  let (background, border, focused_border, radius) = t.text_field_look();
  let (_, menu, lit) = t.menu_look();
  let (off_fill, off_ink) = t.disabled_look();
  let shown = selected
    .and_then(|i| options.get(i).cloned())
    .unwrap_or_else(|| placeholder.clone());
  let quiet = selected.is_none();
  let ink = if !enabled {
    off_ink
  } else if quiet {
    t.colors.on_surface_variant
  } else {
    t.colors.on_surface
  };
  let mut inside: Vec<View> = vec![
    Element::new(ElementKind::Box)
      .style(Style {
        grow: 1.0,
        text_color: Some(ink),
        ..Style::default()
      })
      .children([View::text(shown.clone())])
      .into(),
    Element::new(ElementKind::Box)
      .style(Style {
        text_color: Some(if enabled {
          t.colors.on_surface_variant
        } else {
          off_ink
        }),
        ..Style::default()
      })
      .children([dovetail_image::icons::chevron_down()
        .picture()
        .size(12.0)
        .into()])
      .into(),
  ];
  if is_open {
    // Behind the list: a click anywhere else closes it.
    inside.push(
      Element::new(ElementKind::Box)
        .style(Style {
          layer: Some(Layer::Fill),
          ..Style::default()
        })
        .on_press(Callback::new(move |()| open.set(false)))
        .into(),
    );
    let lit_now = highlight.get();
    let rows: Vec<View> = options
      .iter()
      .enumerate()
      .map(|(i, text)| {
        let choose = choose.clone();
        Element::new(ElementKind::Box)
          .style(Style {
            padding: Edges::xy(t.spacing.of(2.5), t.spacing.of(1.25)),
            radius: 4.0,
            background: if lit_now == Some(i) {
              lit
            } else {
              Color::TRANSPARENT
            },
            text_color: Some(t.colors.on_surface),
            font_weight: Some(if selected == Some(i) {
              dovetail::FontWeight::SEMIBOLD
            } else {
              dovetail::FontWeight::REGULAR
            }),
            ..Style::default()
          })
          .role(Role::ListBoxOption)
          .toggled(selected == Some(i))
          .on_hover(Callback::new(move |over| {
            if over {
              highlight.set(Some(i));
            }
          }))
          .on_press(Callback::new(move |()| choose(i, open)))
          .children([View::text(text.clone())])
          .into()
      })
      .collect();
    inside.push(
      Element::new(ElementKind::Column)
        .style(Style {
          layer: Some(Layer::Below),
          min_width: width,
          max_height: 320.0,
          padding: Edges::all(4.0),
          gap: 1.0,
          align: Align::Stretch,
          background: menu,
          border: Border {
            width: 1.0,
            color: t.colors.outline_variant,
          },
          radius: 6.0,
          clip: true,
          scroll: true,
          ..Style::default()
        })
        .role(Role::ListBox)
        .label(label.clone())
        // Clicks on the list's own space do nothing.
        .on_press(Callback::new(|()| {}))
        .children(rows)
        .into(),
    );
  }
  let control = Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fixed(width),
      padding: Edges::xy(8.0, 5.0),
      gap: t.spacing.of(2.0),
      align: Align::Center,
      justify: Justify::SpaceBetween,
      background: if enabled { background } else { off_fill },
      border: Border {
        width: 1.0,
        color: if is_open { focused_border } else { border },
      },
      radius,
      ..Style::default()
    })
    .role(Role::ComboBox)
    .label(label)
    .text_value(if quiet { String::new() } else { shown })
    .expanded(is_open)
    .children(inside);
  if enabled {
    control
      .focusable(true)
      .on_key(keys)
      .on_press(Callback::new(move |()| {
        if !open.get() {
          highlight.set(selected.or(Some(0)));
        }
        open.set(!open.get());
      }))
      .into()
  } else {
    control.disabled(true).into()
  }
}
