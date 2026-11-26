//! Smaller pieces (M15): a status bar, tabs, a collapsible section, a
//! separator and a link.

use dovetail::{
  Align, Border, Callback, Children, Color, Edges, Element, ElementKind,
  FocusTarget, KeyCode, KeyHandler, NamedKey, Role, Size, Style, View,
  current_component, use_animated,
};
use dovetail_macros::component;
use dovetail_theme::theme;

/// A bar along the bottom of a window that shows what is going on, such
/// as the state of a document or the place of the pointer. Put it last in
/// the window's column. Its children sit side by side.
#[component]
pub fn StatusBar(children: Children) -> View {
  let t = theme();
  let (bar, _, _) = t.menu_look();
  Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fill,
      padding: Edges::xy(t.spacing.of(3.0), t.spacing.of(1.0)),
      gap: t.spacing.of(4.0),
      align: Align::Center,
      background: bar,
      text_color: Some(t.colors.on_surface_variant),
      font_size: Some(t.text.body_small.size),
      font_weight: Some(t.text.body_small.weight),
      ..Style::default()
    })
    .role(Role::StatusBar)
    .label("Status")
    .children(children)
    .into()
}

/// A thin line between groups of things: across, or up and down with
/// `vertical` (as between toolbar buttons).
#[component]
pub fn Separator(#[prop(default)] vertical: bool) -> View {
  let line = theme().colors.outline_variant;
  Element::new(ElementKind::Box)
    .style(Style {
      width: if vertical {
        Size::Fixed(1.0)
      } else {
        Size::Fill
      },
      height: if vertical {
        Size::Fill
      } else {
        Size::Fixed(1.0)
      },
      min_height: if vertical { 16.0 } else { 0.0 },
      background: line,
      ..Style::default()
    })
    .role(Role::Separator)
    .into()
}

/// Text that does something when pressed, such as opening a page or
/// another part of the application. It is in the theme's primary colour
/// and takes focus, so Enter presses it.
#[component]
pub fn Link(
  #[prop(into)] text: String,
  #[prop(default = Callback::new(|_: ()| {}))] on_click: Callback<()>,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  let link = Element::new(ElementKind::Box)
    .style(Style {
      text_color: Some(if enabled {
        t.colors.primary
      } else {
        t.disabled_look().1
      }),
      font_weight: Some(dovetail::FontWeight::MEDIUM),
      ..Style::default()
    })
    .role(Role::Link)
    .children([View::text(text)]);
  if enabled {
    link.focusable(true).on_press(on_click).into()
  } else {
    link.disabled(true).into()
  }
}

/// A section with a title that opens and closes. Clicking the title, or
/// Enter or Space while it has focus, calls `on_toggle` with the new
/// state; the component that owns it updates `open`. What is inside shows
/// only while it is open.
///
/// The content fades and slides in as it opens, and out as it closes,
/// over the theme's medium [`Motion`](dovetail_theme::Motion) time, unless
/// less motion is set (M18). It is laid out at its full size at once.
#[component]
pub fn Collapsible(
  #[prop(into)] title: String,
  open: bool,
  #[prop(default = Callback::new(|_: bool| {}))] on_toggle: Callback<bool>,
  children: Children,
) -> View {
  let t = theme();
  let header: View = Element::new(ElementKind::Row)
    .style(Style {
      gap: t.spacing.of(1.5),
      padding: Edges::xy(0.0, t.spacing.of(1.0)),
      align: Align::Center,
      font_size: Some(t.text.title.size),
      font_weight: Some(t.text.title.weight),
      ..Style::default()
    })
    .role(Role::Button)
    .label(title.clone())
    .expanded(open)
    .focusable(true)
    .on_press(Callback::new(move |()| on_toggle.call(!open)))
    .children([
      Element::new(ElementKind::Box)
        .style(Style {
          width: Size::Fixed(12.0),
          text_color: Some(t.colors.on_surface_variant),
          ..Style::default()
        })
        .children([if open {
          dovetail_image::icons::chevron_down()
        } else {
          dovetail_image::icons::chevron_right()
        }
        .picture()
        .size(12.0)
        .into()])
        .into(),
      View::text(title.clone()),
    ])
    .into();
  // How open it is, from 0 to 1. Closing, the content stays until it has
  // faded out.
  let shown = use_animated(if open { 1.0f32 } else { 0.0 }, t.motion.medium());
  let mut parts = vec![header];
  if open || shown > 0.0 {
    parts.push(
      Element::new(ElementKind::Column)
        .style(Style {
          opacity: shown,
          offset: (0.0, (shown - 1.0) * t.spacing.of(2.0)),
          padding: Edges {
            left: t.spacing.of(4.5),
            top: t.spacing.of(1.0),
            bottom: t.spacing.of(1.0),
            ..Edges::default()
          },
          gap: t.spacing.of(2.0),
          ..Style::default()
        })
        .role(Role::Group)
        .label(title)
        .children(children)
        .into(),
    );
  }
  Element::new(ElementKind::Column)
    .children(parts)
    .into()
}

/// A row of tabs above what the chosen one shows. `children` are what the
/// chosen tab shows; the application builds them for `selected`.
///
/// Clicking a tab calls `on_change` with its position, and the component
/// that owns the choice updates `selected`. Tab reaches the chosen tab, and
/// the arrow keys, Home and End choose another, as on every system.
#[component]
pub fn Tabs(
  tabs: Vec<String>,
  selected: usize,
  // An icon before each tab's title (M17); a tab with `None`, or past the
  // end, has none.
  #[prop(default)] icons: Vec<Option<dovetail_image::Icon>>,
  #[prop(default = Callback::new(|_: usize| {}))] on_change: Callback<usize>,
  children: Children,
) -> View {
  let t = theme();
  let (bar, _, line) = t.tab_look();
  let count = tabs.len();
  let owner = current_component();
  let target =
    move |i: usize| FocusTarget::named(&format!("dovetail.tab.{owner:?}.{i}"));
  let row: Vec<View> = tabs
    .iter()
    .enumerate()
    .map(|(i, title)| {
      let on = i == selected;
      let on_change = on_change.clone();
      Element::new(ElementKind::Box)
        .style(Style {
          padding: Edges::xy(t.spacing.of(3.5), t.spacing.of(1.75)),
          background: if on {
            t.colors.surface
          } else {
            Color::TRANSPARENT
          },
          border: Border {
            width: if on { 1.0 } else { 0.0 },
            color: line,
          },
          radius: 4.0,
          text_color: Some(if on {
            t.colors.on_surface
          } else {
            t.colors.on_surface_variant
          }),
          font_weight: Some(if on {
            dovetail::FontWeight::SEMIBOLD
          } else {
            dovetail::FontWeight::REGULAR
          }),
          ..Style::default()
        })
        .role(Role::Tab)
        .toggled(on)
        .focusable(on)
        .focus_target(target(i))
        .on_press(Callback::new(move |()| {
          on_change.call(i);
          // The chosen tab is the one that takes focus.
          target(i).focus();
        }))
        .children([crate::with_icon(
          icons.get(i).cloned().flatten(),
          title.clone(),
          t.spacing.of(1.5),
        )])
        .into()
    })
    .collect();
  let keys = KeyHandler::new(move |event| {
    if count == 0 {
      return false;
    }
    let next = match &event.key {
      KeyCode::Named(NamedKey::ArrowRight) => (selected + 1) % count,
      KeyCode::Named(NamedKey::ArrowLeft) => (selected + count - 1) % count,
      KeyCode::Named(NamedKey::Home) => 0,
      KeyCode::Named(NamedKey::End) => count - 1,
      _ => return false,
    };
    on_change.call(next);
    target(next).focus();
    true
  });
  let list: View = Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fill,
      gap: 2.0,
      padding: Edges::all(3.0),
      background: bar,
      radius: 6.0,
      ..Style::default()
    })
    .role(Role::TabList)
    .on_key(keys)
    .children(row)
    .into();
  let panel: View = Element::new(ElementKind::Column)
    .style(Style {
      width: Size::Fill,
      padding: Edges::all(t.spacing.of(3.0)),
      gap: t.spacing.of(2.0),
      ..Style::default()
    })
    .role(Role::TabPanel)
    .label(tabs.get(selected).cloned().unwrap_or_default())
    .children(children)
    .into();
  Element::new(ElementKind::Column)
    .style(Style {
      gap: t.spacing.of(1.0),
      width: Size::Fill,
      ..Style::default()
    })
    .children([list, panel])
    .into()
}
