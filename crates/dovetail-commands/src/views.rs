//! Where commands appear: shortcut scopes, toolbar buttons, the menu bar
//! and command search.

use dovetail::{
  Align, Border, Callback, Children, Color, Edges, Element, ElementKind,
  KeyCode, KeyHandler, Layer, MenuItemSpec, MenuModel, MenuSpec, NamedKey,
  Role, Signal, Size, Style, View, native_menus, use_state,
};
use dovetail_controls::{Button, IconButton, TextField};
use dovetail_macros::{component, ui};
use dovetail_theme::theme;

use crate::commands::{CommandInfo, Commands};

/// Text for things that cannot be used now.
/// Colours from the nearest theme (M14), named for what they are here.
fn greyed() -> Color {
  let (_, text) = theme().disabled_look();
  text
}
/// A highlighted menu item.
fn highlight_colour() -> Color {
  theme().menu_look().2
}
/// The menu bar, and separators.
pub(crate) fn bar_colour() -> Color {
  theme().menu_look().0
}
/// Menu text.
fn ink() -> Color {
  theme().colors.on_surface
}
/// Quieter text, such as shortcuts.
fn quiet() -> Color {
  theme().colors.on_surface_variant
}
/// An open menu or command search.
pub(crate) fn menu_background() -> Color {
  theme().menu_look().1
}
/// Menu borders.
pub(crate) fn edge() -> Color {
  theme().colors.outline_variant
}

/// Runs commands from their shortcuts, for everything inside it. Put it
/// round the whole window's content. A focused text field still gets its
/// own keys first, such as Ctrl+Z to undo typing.
///
/// It fills the space it is given.
#[component]
pub fn CommandScope(
  commands: Commands,
  children: Children,
) -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      ..Style::default()
    })
    .on_key(KeyHandler::new(move |event| commands.handle_key(event)))
    .children(children)
    .into()
}

/// A button that runs a command: it shows the command's name and icon, and
/// is greyed out while the command is disabled. With `icon_only`, a command
/// with an icon shows only the icon, as on a toolbar, and its name is the
/// button's name for assistive tools.
#[component]
pub fn CommandButton(
  commands: Commands,
  #[prop(into)] id: String,
  #[prop(default)] icon_only: bool,
) -> View {
  let Some(info) = commands.info(&id) else {
    return View::Empty;
  };
  if icon_only && let Some(icon) = info.icon {
    return ui! {
      IconButton {
        icon: icon,
        label: info.name,
        enabled: info.enabled,
        on_click: move |_| {
          commands.run(&id);
        },
      }
    };
  }
  ui! {
    Button {
      text: info.name,
      enabled: info.enabled,
      icon: info.icon,
      on_click: move |_| {
        commands.run(&id);
      },
    }
  }
}

/// A row of tool buttons, such as [`CommandButton`]s.
#[component]
pub fn Toolbar(children: Children) -> View {
  Element::new(ElementKind::Row)
    .style(Style {
      gap: 6.0,
      padding: Edges::xy(8.0, 6.0),
      align: Align::Center,
      ..Style::default()
    })
    .role(Role::Toolbar)
    .label("Toolbar")
    .children(children)
    .into()
}

/// One menu of a [`MenuBar`]: a title and its items.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
  /// The title in the bar.
  pub title: String,
  /// The items, top to bottom.
  pub items: Vec<MenuItem>,
}

/// One item of a [`Menu`].
#[derive(Clone, Debug, PartialEq)]
pub enum MenuItem {
  /// Runs the command with this id.
  Command(String),
  /// A line between groups of items.
  Separator,
}

impl Menu {
  /// A menu with this title and no items yet.
  pub fn new(title: impl Into<String>) -> Menu {
    Menu { title: title.into(), items: Vec::new() }
  }

  /// Adds an item that runs the command `id`.
  pub fn command(
    mut self,
    id: impl Into<String>,
  ) -> Menu {
    self.items.push(MenuItem::Command(id.into()));
    self
  }

  /// Adds a line between groups of items.
  pub fn separator(mut self) -> Menu {
    self.items.push(MenuItem::Separator);
    self
  }
}

/// The menus as the system menu bar takes them.
fn model(
  commands: Commands,
  menus: &[Menu],
) -> MenuModel {
  MenuModel {
    menus: menus
      .iter()
      .map(|m| MenuSpec {
        title: m.title.clone(),
        items: m
          .items
          .iter()
          .filter_map(|item| match item {
            MenuItem::Separator => Some(MenuItemSpec::Separator),
            MenuItem::Command(id) => {
              commands.info(id).map(|c| MenuItemSpec::Command {
                id: c.id,
                label: c.name,
                enabled: c.enabled,
                key: c.shortcut.map(|s| s.menu_key()),
                icon: c.icon.as_ref().map(menu_icon),
              })
            },
          })
          .collect(),
      })
      .collect(),
  }
}

/// A command's icon for the system menu bar: 16 points, drawn at twice
/// that for sharp displays. A one-colour icon is a template, which the
/// system colours as it colours the menu's text (D0024).
pub(crate) fn menu_icon(icon: &dovetail_image::Icon) -> dovetail::MenuIcon {
  const POINTS: f32 = 16.0;
  let side = (POINTS * 2.0) as u32;
  let template = !icon.is_multicolour();
  let pixels: Vec<u8> = if template {
    icon
      .render(side, side)
      .alpha
      .iter()
      .flat_map(|&a| [0, 0, 0, a])
      .collect()
  } else {
    icon
      .render_rgba(side, side, [0.0, 0.0, 0.0, 1.0])
      .pixels
      .as_chunks::<4>()
      .0
      .iter()
      .flat_map(|p| {
        let a = u16::from(p[3]);
        let pre = |c: u8| ((u16::from(c) * a + 127) / 255) as u8;
        [pre(p[0]), pre(p[1]), pre(p[2]), p[3]]
      })
      .collect()
  };
  dovetail::MenuIcon {
    size: POINTS,
    width: side,
    height: side,
    pixels: pixels.into(),
    template,
  }
}

/// The commands of a menu that can be chosen, by item position.
fn choosable(
  commands: Commands,
  menu: &Menu,
) -> Vec<usize> {
  menu
    .items
    .iter()
    .enumerate()
    .filter(|(_, item)| {
      matches!(item, MenuItem::Command(id) if commands.is_enabled(id))
    })
    .map(|(i, _)| i)
    .collect()
}

/// Opens menu `index`, with its first item that can be chosen highlighted.
fn open_menu(
  commands: Commands,
  menus: &[Menu],
  index: usize,
  open: Signal<Option<usize>>,
  highlight: Signal<Option<usize>>,
) {
  let Some(menu) = menus.get(index) else {
    return;
  };
  highlight.set(choosable(commands, menu).first().copied());
  open.set(Some(index));
}

/// A menu bar. On systems with a menu bar of their own, such as macOS, the
/// menus go there and nothing is drawn in the window. Elsewhere it draws
/// the menu titles; choosing one opens its menu below it.
///
/// With the keyboard, F10 opens the first menu; the arrow keys move
/// between items and menus, Enter chooses, and Escape closes. Items show
/// their commands' shortcuts, and disabled commands are greyed out.
#[component]
pub fn MenuBar(
  commands: Commands,
  menus: Vec<Menu>,
) -> View {
  if let Some(host) = native_menus() {
    host.set_menus(
      &model(commands, &menus),
      Callback::new(move |id: String| {
        commands.run(&id);
      }),
    );
    return View::Empty;
  }
  let open = use_state(|| None::<usize>);
  let highlight = use_state(|| None::<usize>);
  {
    let menus = menus.clone();
    use_state(move || {
      commands.define(
        "menu.open",
        "Open the menu bar",
        Some("F10"),
        move || open_menu(commands, &menus, 0, open, highlight),
      );
    });
  }

  let open_now = open.get();
  let mut bar: Vec<View> = Vec::new();
  if open_now.is_some() {
    // Behind the open menu: a click anywhere else closes it.
    bar.push(
      Element::new(ElementKind::Box)
        .style(Style {
          layer: Some(Layer::Fill),
          ..Style::default()
        })
        .on_press(Callback::new(move |()| open.set(None)))
        .into(),
    );
  }
  for (i, menu) in menus.iter().enumerate() {
    let is_open = open_now == Some(i);
    let mut inside = vec![View::text(menu.title.clone())];
    if is_open {
      inside.push(drop_down(commands, &menus, i, open, highlight));
    }
    let menus = menus.clone();
    bar.push(
      Element::new(ElementKind::Box)
        .style(Style {
          padding: Edges::xy(10.0, 4.0),
          radius: 4.0,
          background: if is_open {
            highlight_colour()
          } else {
            Color::TRANSPARENT
          },
          ..Style::default()
        })
        .role(Role::MenuItem)
        .label(menu.title.clone())
        .on_press(Callback::new(move |()| {
          if open.get() == Some(i) {
            open.set(None);
          } else {
            open_menu(commands, &menus, i, open, highlight);
          }
        }))
        .children(inside)
        .into(),
    );
  }
  Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fill,
      gap: 2.0,
      padding: Edges::xy(4.0, 2.0),
      background: bar_colour(),
      text_color: Some(ink()),
      font_size: Some(14.0),
      ..Style::default()
    })
    .role(Role::MenuBar)
    .label("Menu bar")
    .children(bar)
    .into()
}

/// The open menu `index`, as a layer below its title.
fn drop_down(
  commands: Commands,
  menus: &[Menu],
  index: usize,
  open: Signal<Option<usize>>,
  highlight: Signal<Option<usize>>,
) -> View {
  let menu = &menus[index];
  let lit = highlight.get();
  let rows: Vec<View> = menu
    .items
    .iter()
    .enumerate()
    .map(|(i, item)| match item {
      MenuItem::Separator => Element::new(ElementKind::Box)
        .style(Style {
          height: Size::Fixed(1.0),
          background: bar_colour(),
          ..Style::default()
        })
        .into(),
      MenuItem::Command(id) => {
        let info = commands.info(id).unwrap_or(CommandInfo {
          id: id.clone(),
          name: id.clone(),
          shortcut: None,
          default_shortcut: None,
          enabled: false,
          icon: None,
        });
        menu_row(commands, &info, lit == Some(i), Some(open))
      },
    })
    .collect();
  let keys = {
    let menus = menus.to_vec();
    KeyHandler::new(move |event| {
      menu_key(commands, &menus, index, open, highlight, event)
    })
  };
  Element::new(ElementKind::Column)
    .style(Style {
      layer: Some(Layer::Below),
      min_width: 200.0,
      padding: Edges::all(4.0),
      gap: 2.0,
      align: Align::Stretch,
      background: menu_background(),
      border: Border { width: 1.0, color: edge() },
      radius: 6.0,
      ..Style::default()
    })
    .role(Role::Menu)
    .label(menu.title.clone())
    .focusable(true)
    .autofocus(true)
    .on_key(keys)
    // Clicks on the menu's own space do nothing, rather than reaching
    // the title behind it.
    .on_press(Callback::new(|()| {}))
    .children(rows)
    .into()
}

/// One item: the command's name and shortcut, highlighted or greyed out.
/// With `close`, choosing it closes the menu first.
pub(crate) fn menu_row(
  commands: Commands,
  info: &CommandInfo,
  lit: bool,
  close: Option<Signal<Option<usize>>>,
) -> View {
  let id = info.id.clone();
  let mut row = Element::new(ElementKind::Row)
    .style(Style {
      padding: Edges::xy(10.0, 5.0),
      gap: 24.0,
      radius: 4.0,
      justify: dovetail::Justify::SpaceBetween,
      background: if lit {
        highlight_colour()
      } else {
        Color::TRANSPARENT
      },
      text_color: Some(if info.enabled { ink() } else { greyed() }),
      ..Style::default()
    })
    .role(Role::MenuItem)
    .label(info.name.clone())
    .children([
      match &info.icon {
        // The icon is decoration: the label names the item.
        Some(icon) => Element::new(ElementKind::Row)
          .style(Style {
            gap: 8.0,
            align: Align::Center,
            ..Style::default()
          })
          .children([
            icon.picture().into(),
            View::text(info.name.clone()),
          ])
          .into(),
        None => View::text(info.name.clone()),
      },
      match &info.shortcut {
        Some(s) => Element::new(ElementKind::Box)
          .style(Style {
            text_color: Some(if info.enabled { quiet() } else { greyed() }),
            ..Style::default()
          })
          .children([View::text(s.label())])
          .into(),
        None => View::Empty,
      },
    ]);
  if info.enabled {
    row = row.on_press(Callback::new(move |()| {
      if let Some(open) = close {
        open.set(None);
      }
      commands.run(&id);
    }));
  } else {
    row = row.disabled(true);
  }
  row.into()
}

/// The arrow keys, Enter and Escape in an open menu.
fn menu_key(
  commands: Commands,
  menus: &[Menu],
  index: usize,
  open: Signal<Option<usize>>,
  highlight: Signal<Option<usize>>,
  event: &dovetail::KeyEvent,
) -> bool {
  let menu = &menus[index];
  let items = choosable(commands, menu);
  let at = highlight
    .get()
    .and_then(|h| items.iter().position(|&i| i == h));
  let step = |forward: bool| {
    if items.is_empty() {
      return None;
    }
    Some(match (at, forward) {
      (None, true) => items[0],
      (None, false) => items[items.len() - 1],
      (Some(p), true) => items[(p + 1) % items.len()],
      (Some(p), false) => items[(p + items.len() - 1) % items.len()],
    })
  };
  match &event.key {
    KeyCode::Named(NamedKey::ArrowDown) => highlight.set(step(true)),
    KeyCode::Named(NamedKey::ArrowUp) => highlight.set(step(false)),
    KeyCode::Named(NamedKey::ArrowRight) => {
      open_menu(commands, menus, (index + 1) % menus.len(), open, highlight)
    },
    KeyCode::Named(NamedKey::ArrowLeft) => open_menu(
      commands,
      menus,
      (index + menus.len() - 1) % menus.len(),
      open,
      highlight,
    ),
    KeyCode::Named(NamedKey::Escape | NamedKey::Tab) => open.set(None),
    KeyCode::Named(NamedKey::Enter | NamedKey::Space) => {
      if let Some(MenuItem::Command(id)) =
        highlight.get().and_then(|h| menu.items.get(h))
      {
        open.set(None);
        commands.run(id);
      }
    },
    _ => return false,
  }
  true
}

/// Command search: Mod+Shift+P (or the `commands.search` command) opens a
/// box at the top of the window that finds commands by name as you type.
/// Up and Down choose, Enter runs, and Escape or a click outside closes it.
/// Disabled commands are listed greyed out, and cannot be run.
#[component]
pub fn CommandSearch(commands: Commands) -> View {
  let open = use_state(|| false);
  let query = use_state(String::new);
  let chosen = use_state(|| 0usize);
  use_state(move || {
    commands.define(
      "commands.search",
      "Search commands",
      Some("Mod+Shift+P"),
      move || {
        query.set(String::new());
        chosen.set(0);
        open.set(true);
      },
    );
  });
  if !open.get() {
    return View::Empty;
  }
  let results = found(commands, &query.get());
  let at = chosen.get().min(results.len().saturating_sub(1));
  let rows: Vec<View> = results
    .iter()
    .enumerate()
    .map(|(i, c)| {
      let close = move || open.set(false);
      let id = c.id.clone();
      let row = menu_row(commands, c, i == at, None);
      // Choosing closes the search first.
      match row {
        View::Element(e) if c.enabled => {
          let mut e = (*e).clone();
          e.role = Role::ListBoxOption;
          e.on_press = Some(Callback::new(move |()| {
            close();
            commands.run(&id);
          }));
          e.into()
        },
        View::Element(e) => {
          let mut e = (*e).clone();
          e.role = Role::ListBoxOption;
          e.into()
        },
        other => other,
      }
    })
    .collect();
  let keys = KeyHandler::new(move |event| {
    let results = found(commands, &query.get());
    match &event.key {
      KeyCode::Named(NamedKey::ArrowDown) if !results.is_empty() => {
        chosen.set((chosen.get().min(results.len() - 1) + 1) % results.len())
      },
      KeyCode::Named(NamedKey::ArrowUp) if !results.is_empty() => {
        let c = chosen.get().min(results.len() - 1);
        chosen.set((c + results.len() - 1) % results.len());
      },
      KeyCode::Named(NamedKey::Enter) => {
        if let Some(c) =
          results.get(chosen.get().min(results.len().saturating_sub(1)))
          && c.enabled
        {
          open.set(false);
          commands.run(&c.id);
        }
      },
      KeyCode::Named(NamedKey::Escape) => open.set(false),
      _ => return false,
    }
    true
  });
  let backdrop: View = Element::new(ElementKind::Box)
    .style(Style {
      layer: Some(Layer::Fill),
      background: theme().colors.scrim,
      ..Style::default()
    })
    .on_press(Callback::new(move |()| open.set(false)))
    .into();
  let field = ui! {
    TextField {
      value: query.get(),
      on_change: move |v| {
        query.set(v);
        chosen.set(0);
      },
      width: 404.0,
      label: "Search commands",
      autofocus: true,
    }
  };
  let list: View = Element::new(ElementKind::Column)
    .style(Style {
      gap: 2.0,
      align: Align::Stretch,
      text_color: Some(ink()),
      font_size: Some(14.0),
      ..Style::default()
    })
    .role(Role::ListBox)
    .label("Commands found")
    .children(if rows.is_empty() {
      vec![View::text("No commands match.")]
    } else {
      rows
    })
    .into();
  let panel: View = Element::new(ElementKind::Column)
    .style(Style {
      layer: Some(Layer::TopCentre(60.0)),
      modal: true,
      width: Size::Fixed(420.0),
      padding: Edges::all(8.0),
      gap: 6.0,
      align: Align::Stretch,
      background: menu_background(),
      border: Border { width: 1.0, color: edge() },
      radius: 8.0,
      ..Style::default()
    })
    .role(Role::Group)
    .label("Search commands")
    .on_key(keys)
    .on_press(Callback::new(|()| {}))
    .children([field, list])
    .into();
  View::fragment([backdrop, panel])
}

/// What command search lists for `query`: at most eight matches, without
/// command search itself.
fn found(
  commands: Commands,
  query: &str,
) -> Vec<CommandInfo> {
  commands
    .search(query)
    .into_iter()
    .filter(|c| c.id != "commands.search")
    .take(8)
    .collect()
}
