//! Context menus (M15, D0021): a menu of commands opened where the user
//! right-clicks, or with the Menu key or Shift+F10.

use dovetail::{
  Align, Border, Callback, Children, Edges, Element, ElementKind, KeyCode,
  KeyHandler, Layer, NamedKey, Role, Signal, Size, Style, View, use_state,
};
use dovetail_macros::component;
use dovetail_theme::theme;

use crate::commands::{CommandInfo, Commands};
use crate::views::{MenuItem, bar_colour, edge, menu_background, menu_row};

/// The positions of the items that can be chosen now.
fn choosable(
  commands: Commands,
  items: &[MenuItem],
) -> Vec<usize> {
  items
    .iter()
    .enumerate()
    .filter(|(_, item)| {
      matches!(item, MenuItem::Command(id) if commands.is_enabled(id))
    })
    .map(|(i, _)| i)
    .collect()
}

/// Gives what is inside it a menu of commands. A right-click inside opens
/// it at the pointer, and the Menu key or Shift+F10 opens it below the
/// focused element.
///
/// The items are the same [`MenuItem`]s as a [`crate::MenuBar`]'s, with
/// the commands' names and shortcuts. Disabled commands are greyed out and
/// cannot be chosen. In the open menu the arrow keys move the highlight,
/// Enter chooses, and Escape or a click outside closes it.
#[component]
pub fn ContextMenu(
  commands: Commands,
  items: Vec<MenuItem>,
  #[prop(into, default = "Context menu")] label: String,
  children: Children,
) -> View {
  let open = use_state(|| None::<(f32, f32)>);
  let highlight = use_state(|| None::<usize>);
  let mut inside: Vec<View> = children;
  if let Some(at) = open.get() {
    inside.extend(menu(commands, &items, &label, at, open, highlight));
  }
  Element::new(ElementKind::Box)
    .on_context_menu(Callback::new(move |at| {
      highlight.set(None);
      open.set(Some(at));
    }))
    .children(inside)
    .into()
}

/// One row of the open menu. A row that can be chosen closes the menu
/// before it runs its command, and is highlighted under the pointer.
fn row(
  commands: Commands,
  item: &MenuItem,
  i: usize,
  open: Signal<Option<(f32, f32)>>,
  highlight: Signal<Option<usize>>,
) -> View {
  let id = match item {
    MenuItem::Separator => {
      return Element::new(ElementKind::Box)
        .style(Style {
          height: Size::Fixed(1.0),
          background: bar_colour(),
          ..Style::default()
        })
        .role(Role::Separator)
        .into();
    },
    MenuItem::Command(id) => id,
  };
  let info = commands.info(id).unwrap_or(CommandInfo {
    id: id.clone(),
    name: id.clone(),
    shortcut: None,
    default_shortcut: None,
    enabled: false,
    icon: None,
  });
  let lit = highlight.get() == Some(i);
  match menu_row(commands, &info, lit, None) {
    View::Element(e) if e.on_press.is_some() => {
      let mut e = (*e).clone();
      let run = e.on_press.take();
      e.on_press = Some(Callback::new(move |()| {
        open.set(None);
        if let Some(run) = &run {
          run.call(());
        }
      }));
      e.on_hover = Some(Callback::new(move |over| {
        if over {
          highlight.set(Some(i));
        }
      }));
      e.into()
    },
    other => other,
  }
}

/// The open menu at (`x`, `y`) in the window, and a backdrop behind it
/// that closes it.
fn menu(
  commands: Commands,
  items: &[MenuItem],
  label: &str,
  (x, y): (f32, f32),
  open: Signal<Option<(f32, f32)>>,
  highlight: Signal<Option<usize>>,
) -> [View; 2] {
  let rows: Vec<View> = items
    .iter()
    .enumerate()
    .map(|(i, item)| row(commands, item, i, open, highlight))
    .collect();
  let keys = {
    let items = items.to_vec();
    KeyHandler::new(move |event| {
      let can = choosable(commands, &items);
      let at = highlight
        .get()
        .and_then(|h| can.iter().position(|&i| i == h));
      let step = |forward: bool| {
        if can.is_empty() {
          return None;
        }
        Some(match (at, forward) {
          (None, true) => can[0],
          (None, false) => can[can.len() - 1],
          (Some(p), true) => can[(p + 1) % can.len()],
          (Some(p), false) => can[(p + can.len() - 1) % can.len()],
        })
      };
      match &event.key {
        KeyCode::Named(NamedKey::ArrowDown) => highlight.set(step(true)),
        KeyCode::Named(NamedKey::ArrowUp) => highlight.set(step(false)),
        KeyCode::Named(NamedKey::Home) => highlight.set(can.first().copied()),
        KeyCode::Named(NamedKey::End) => highlight.set(can.last().copied()),
        KeyCode::Named(NamedKey::Escape | NamedKey::Tab) => open.set(None),
        KeyCode::Named(NamedKey::Enter | NamedKey::Space) => {
          if let Some(MenuItem::Command(id)) =
            highlight.get().and_then(|h| items.get(h))
            && commands.is_enabled(id)
          {
            open.set(None);
            commands.run(id);
          }
        },
        _ => return false,
      }
      true
    })
  };
  let backdrop: View = Element::new(ElementKind::Box)
    .style(Style {
      layer: Some(Layer::Fill),
      ..Style::default()
    })
    .on_press(Callback::new(move |()| open.set(None)))
    .into();
  let panel: View = Element::new(ElementKind::Column)
    .style(Style {
      layer: Some(Layer::At(x, y)),
      min_width: 200.0,
      padding: Edges::all(4.0),
      gap: 2.0,
      align: Align::Stretch,
      background: menu_background(),
      border: Border { width: 1.0, color: edge() },
      radius: 6.0,
      text_color: Some(theme().colors.on_surface),
      ..Style::default()
    })
    .role(Role::Menu)
    .label(label.to_owned())
    .focusable(true)
    .autofocus(true)
    .on_key(keys)
    // Clicks on the menu's own space do nothing.
    .on_press(Callback::new(|()| {}))
    .children(rows)
    .into();
  [backdrop, panel]
}
