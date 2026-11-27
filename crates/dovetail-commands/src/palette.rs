//! Tool palettes, as in drawing programs (M25, D0030).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use dovetail::{
  Align, Border, Callback, Color, Edges, Element, ElementKind, FocusTarget,
  Justify, KeyCode, KeyHandler, Layer, NamedKey, PointerButton, PointerHandler,
  PointerKind, Role, Size, Style, Task, View, current_component, use_state,
};
use dovetail_controls::Tooltip;
use dovetail_macros::{component, ui};
use dovetail_theme::theme;

use crate::commands::{CommandInfo, Commands};
use crate::shortcut::{Shortcut, ShortcutKey};
use crate::views::{edge, menu_background};

/// How long a press on a grouped tool is held before its group opens.
pub const LONG_PRESS: Duration = Duration::from_millis(500);

/// The size of a tool's icon.
const ICON: f32 = 20.0;
/// Room round a tool's icon.
const PAD: f32 = 6.0;
/// The space between tools.
const GAP: f32 = 2.0;

/// The tool a slot shows: the one of its tools that is chosen, or else the
/// one used last.
fn shown(
  commands: Commands,
  slot: &[String],
  last: usize,
) -> usize {
  slot
    .iter()
    .position(|id| commands.is_on(id) == Some(true))
    .unwrap_or(last.min(slot.len().saturating_sub(1)))
}

/// The plain letter of a tool's shortcut, such as `b` for `B`.
fn letter(info: &CommandInfo) -> Option<char> {
  match &info.shortcut {
    Some(Shortcut {
      key: ShortcutKey::Char(c),
      primary: false,
      alt: false,
      shift: false,
    }) => Some(*c),
    _ => None,
  }
}

/// A palette of tools, as in drawing programs (M25).
///
/// `tools` are slots of command ids, each a choice of one group, so one
/// tool is chosen (see [`Commands::define_choice`]). A slot with one id is
/// one tool. A slot with several is a group: its button shows the tool of
/// the group used last, with a mark in its corner, and a long press, a
/// right-click, the Menu key or Alt+Down opens the rest of the group.
///
/// Each tool runs from its own shortcut, such as `B`, from anywhere but a
/// text field. Shift with the first tool's letter goes through a group's
/// tools. Tooltips give each tool's name and key.
///
/// The buttons go in `columns` columns, or, without, in as many as fit
/// across, so the palette is a column in a narrow panel and a row in a
/// wide one. It is one Tab stop: the arrow keys move between tools, and
/// Space or Enter chooses.
#[component]
pub fn ToolPalette(
  commands: Commands,
  tools: Vec<Vec<String>>,
  #[prop(default)] columns: Option<usize>,
  #[prop(into, default = "Tools")] label: String,
) -> View {
  let t = theme();
  let look = t.group_look();
  let slots = tools.len();
  // The tool each slot showed last, kept as it is built: not reactive, as
  // only building reads it.
  let last = use_state(|| Rc::new(RefCell::new(Vec::<usize>::new()))).get();
  let open = use_state(|| None::<usize>);
  let lit = use_state(|| 0usize);
  let at = use_state(|| None::<usize>);
  let width = use_state(|| f32::INFINITY);
  let held = use_state(|| None::<Task>);
  let long_pressed = use_state(|| false);
  // Shift and a group's letter goes through its tools.
  {
    let tools = tools.clone();
    use_state(move || {
      for (s, slot) in tools.iter().enumerate() {
        let Some(c) = slot
          .first()
          .and_then(|id| commands.info(id))
          .and_then(|i| letter(&i))
        else {
          continue;
        };
        if slot.len() < 2 {
          continue;
        }
        let slot = slot.clone();
        commands.define(
          format!("palette.next.{}", slot[0]),
          format!("Next tool in group {}", s + 1),
          Some(&format!("Shift+{c}")),
          move || {
            let i = shown(commands, &slot, 0);
            commands.run(&slot[(i + 1) % slot.len()]);
          },
        );
      }
    });
  }

  let shows: Vec<usize> = {
    let mut last = last.borrow_mut();
    last.resize(slots, 0);
    tools
      .iter()
      .enumerate()
      .map(|(s, slot)| {
        let i = shown(commands, slot, last[s]);
        last[s] = i;
        i
      })
      .collect()
  };
  let chosen_slot = (0..slots).find(|&s| {
    tools[s]
      .get(shows[s])
      .is_some_and(|id| commands.is_on(id) == Some(true))
  });
  let tab_stop = at
    .get()
    .filter(|&s| s < slots)
    .or(chosen_slot)
    .unwrap_or(0);
  let step = ICON + 2.0 * PAD + GAP;
  let across = columns
    .unwrap_or_else(|| {
      ((width.get() - 2.0 * GAP + GAP) / step)
        .floor()
        .max(1.0) as usize
    })
    .clamp(1, slots.max(1));
  let owner = current_component();
  let target = move |s: usize| {
    FocusTarget::named(&format!("dovetail.palette.{owner:?}.{s}"))
  };

  let open_group = move |s: usize, from: &[Vec<String>]| {
    if from.get(s).is_some_and(|slot| slot.len() > 1) {
      lit.set(0);
      open.set(Some(s));
    }
  };

  let mut buttons: Vec<View> = Vec::with_capacity(slots);
  for (s, slot) in tools.iter().enumerate() {
    let Some(id) = slot.get(shows[s]).cloned() else {
      buttons.push(View::Empty);
      continue;
    };
    let info = commands
      .info(&id)
      .unwrap_or_else(|| CommandInfo::missing(&id));
    let on = info.on == Some(true);
    let grouped = slot.len() > 1;
    let fill = if on { look.pressed } else { Color::TRANSPARENT };
    let ink = if !info.enabled {
      t.disabled_look().1
    } else if on {
      look.pressed_text
    } else {
      look.text
    };
    let icon: View = match &info.icon {
      Some(icon) => icon.picture().size(ICON).into(),
      None => View::text(info.name.chars().take(2).collect::<String>()),
    };
    let face: View = Element::new(ElementKind::Row)
      .style(Style {
        width: Size::Fixed(ICON),
        height: Size::Fixed(ICON),
        align: Align::Center,
        justify: Justify::Center,
        ..Style::default()
      })
      .children([icon])
      .into();
    let mut inside = vec![face];
    if grouped {
      // The mark that says more tools are behind this one.
      inside.push(
        Element::new(ElementKind::Box)
          .style(Style {
            width: Size::Fixed(4.0),
            height: Size::Fixed(4.0),
            radius: 1.0,
            background: ink,
            offset: (PAD - 2.0, PAD - 2.0),
            ..Style::default()
          })
          .into(),
      );
    }
    if open.get() == Some(s) {
      inside.push(group_layer(commands, slot, s, lit, open, target));
    }
    let stack: View = Element::new(ElementKind::Stack)
      .style(Style {
        width: Size::Fixed(ICON),
        height: Size::Fixed(ICON),
        align: Align::End,
        ..Style::default()
      })
      .children(inside)
      .into();
    let run_id = id.clone();
    let mut button = Element::new(ElementKind::Box)
      .style(Style {
        padding: Edges::all(PAD),
        radius: look.radius,
        background: fill,
        text_color: Some(ink),
        ..Style::default()
      })
      .role(Role::RadioButton)
      .label(info.name.clone())
      .toggled(on)
      .focus_target(target(s))
      .children([stack]);
    if grouped {
      let from = tools.clone();
      let from_menu = tools.clone();
      button = button
        .expanded(open.get() == Some(s))
        .on_pointer(PointerHandler::new(move |e| {
          if e.button != PointerButton::Primary {
            return;
          }
          match e.kind {
            PointerKind::Down => {
              long_pressed.set(false);
              let from = from.clone();
              held.set(Some(Task::after(
                LONG_PRESS,
                Callback::new(move |()| {
                  // The palette may have gone while it waited.
                  if held.try_update(|h| *h = None).is_err() {
                    return;
                  }
                  long_pressed.set(true);
                  open_group(s, &from);
                }),
              )));
            },
            PointerKind::Up => {
              if let Some(task) = held.get() {
                task.cancel();
                held.set(None);
              }
            },
            _ => {},
          }
        }))
        .on_context_menu(Callback::new(move |_| open_group(s, &from_menu)));
    }
    if info.enabled {
      button = button
        .focusable(s == tab_stop)
        .on_press(Callback::new(move |()| {
          // The release after a long press only ends it.
          if long_pressed.get() {
            long_pressed.set(false);
            return;
          }
          commands.run(&run_id);
          at.set(Some(s));
          target(s).focus();
        }));
    } else {
      button = button.disabled(true);
    }
    let tip = match (letter(&info), slot.first().and_then(|f| commands.info(f)))
    {
      (Some(c), _) => format!("{} ({})", info.name, c.to_ascii_uppercase()),
      (None, Some(first)) if grouped => match letter(&first) {
        Some(c) => format!("{} (Shift+{})", info.name, c.to_ascii_uppercase()),
        None => info.name.clone(),
      },
      _ => info.name.clone(),
    };
    let button: View = button.into();
    if open.get() == Some(s) {
      // The open group is in the way of a tooltip, and focus inside it
      // would show one over it.
      buttons.push(button);
    } else {
      buttons.push(ui! { Tooltip { text: tip, { button } } });
    }
  }

  let keys = {
    let tools = tools.clone();
    KeyHandler::new(move |event| {
      if slots == 0 || open.get().is_some() {
        return false;
      }
      let from = at.get().unwrap_or(tab_stop).min(slots - 1);
      let alt = event.modifiers.alt;
      let next = match &event.key {
        KeyCode::Named(NamedKey::ArrowDown) if alt => {
          open_group(from, &tools);
          return true;
        },
        KeyCode::Named(NamedKey::ArrowRight) => (from + 1).min(slots - 1),
        KeyCode::Named(NamedKey::ArrowLeft) => from.saturating_sub(1),
        KeyCode::Named(NamedKey::ArrowDown) => {
          if from + across < slots {
            from + across
          } else {
            from
          }
        },
        KeyCode::Named(NamedKey::ArrowUp) => {
          from.checked_sub(across).unwrap_or(from)
        },
        KeyCode::Named(NamedKey::Home) => 0,
        KeyCode::Named(NamedKey::End) => slots - 1,
        _ => return false,
      };
      at.set(Some(next));
      target(next).focus();
      true
    })
  };
  let rows: Vec<View> = buttons
    .chunks(across)
    .map(|row| {
      Element::new(ElementKind::Row)
        .style(Style { gap: GAP, ..Style::default() })
        .children(row.to_vec())
        .into()
    })
    .collect();
  Element::new(ElementKind::Column)
    .style(Style {
      width: if columns.is_some() {
        Size::Fit
      } else {
        Size::Fill
      },
      padding: Edges::all(GAP),
      gap: GAP,
      ..Style::default()
    })
    .role(Role::Toolbar)
    .label(label)
    .on_key(keys)
    .on_size(Callback::new(move |(w, _): (f32, f32)| {
      if width.get() != w {
        width.set(w);
      }
    }))
    .children(rows)
    .into()
}

/// The open group of slot `s`: its tools, beside the button.
fn group_layer(
  commands: Commands,
  slot: &[String],
  s: usize,
  lit: dovetail::Signal<usize>,
  open: dovetail::Signal<Option<usize>>,
  target: impl Fn(usize) -> FocusTarget + Copy + 'static,
) -> View {
  let t = theme();
  let highlight = t.menu_look().2;
  let count = slot.len();
  let choose = move |id: &str| {
    open.set(None);
    commands.run(id);
    target(s).focus();
  };
  let rows: Vec<View> = slot
    .iter()
    .enumerate()
    .map(|(i, id)| {
      let info = commands
        .info(id)
        .unwrap_or_else(|| CommandInfo::missing(id));
      let key = info
        .shortcut
        .as_ref()
        .map(|s| s.label())
        .unwrap_or_default();
      let id = id.clone();
      let mut row = Element::new(ElementKind::Row)
        .style(Style {
          padding: Edges::xy(8.0, 4.0),
          gap: 8.0,
          radius: 4.0,
          align: Align::Center,
          background: if lit.get() == i {
            highlight
          } else {
            Color::TRANSPARENT
          },
          text_color: Some(if info.enabled {
            t.colors.on_surface
          } else {
            t.disabled_look().1
          }),
          ..Style::default()
        })
        .role(Role::MenuItemRadio)
        .label(info.name.clone())
        .toggled(info.on == Some(true))
        .on_hover(Callback::new(move |over| {
          if over {
            lit.set(i);
          }
        }))
        .children([
          match &info.icon {
            Some(icon) => icon.picture().size(16.0).into(),
            None => View::Empty,
          },
          Element::new(ElementKind::Box)
            .style(Style { min_width: 120.0, ..Style::default() })
            .children([View::text(info.name.clone())])
            .into(),
          Element::new(ElementKind::Box)
            .style(Style {
              text_color: Some(t.colors.on_surface_variant),
              ..Style::default()
            })
            .children([View::text(key)])
            .into(),
        ]);
      if info.enabled {
        row = row.on_press(Callback::new(move |()| choose(&id)));
      } else {
        row = row.disabled(true);
      }
      row.into()
    })
    .collect();
  let ids: Vec<String> = slot.to_vec();
  let keys = KeyHandler::new(move |event| {
    match &event.key {
      KeyCode::Named(NamedKey::ArrowDown) => lit.set((lit.get() + 1) % count),
      KeyCode::Named(NamedKey::ArrowUp) => {
        lit.set((lit.get() + count - 1) % count)
      },
      KeyCode::Named(NamedKey::Home) => lit.set(0),
      KeyCode::Named(NamedKey::End) => lit.set(count - 1),
      KeyCode::Named(NamedKey::Enter | NamedKey::Space) => {
        if let Some(id) = ids.get(lit.get())
          && commands.is_enabled(id)
        {
          choose(id);
        }
      },
      KeyCode::Named(NamedKey::Escape | NamedKey::Tab) => {
        open.set(None);
        target(s).focus();
        return event.is(NamedKey::Escape);
      },
      _ => return false,
    }
    true
  });
  let backdrop: View = Element::new(ElementKind::Box)
    .style(Style {
      layer: Some(Layer::Fill),
      ..Style::default()
    })
    .on_press(Callback::new(move |()| open.set(None)))
    .into();
  let menu: View = Element::new(ElementKind::Column)
    .style(Style {
      layer: Some(Layer::Below),
      padding: Edges::all(4.0),
      gap: 2.0,
      align: Align::Stretch,
      background: menu_background(),
      border: Border { width: 1.0, color: edge() },
      radius: 6.0,
      ..Style::default()
    })
    .role(Role::Menu)
    .label("Tools in this group")
    .focusable(true)
    .autofocus(true)
    .on_key(keys)
    .on_press(Callback::new(|()| {}))
    .children(rows)
    .into();
  View::fragment([backdrop, menu])
}
