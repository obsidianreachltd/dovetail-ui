//! Date and time pickers (M25, D0030): a calendar, a date field that opens
//! one, and a time field. Dates are shown year-month-day and names are in
//! English until M22.

use dovetail::{
  Align, Border, Callback, Color, Date, Edges, Element, ElementKind,
  FocusTarget, Justify, KeyCode, KeyEvent, KeyHandler, Layer, MONTH_NAMES,
  NamedKey, Role, Size, Style, Time, View, Weekday, current_component, today,
  use_state,
};
use dovetail_macros::{component, ui};
use dovetail_theme::theme;

use crate::{IconButton, TextField};

/// The size of a day in a calendar.
const DAY: f32 = 32.0;

/// `date` kept between `first` and `last`, where they are given.
fn clamp(
  date: Date,
  first: Option<Date>,
  last: Option<Date>,
) -> Date {
  let date = first.map_or(date, |f| date.max(f));
  last.map_or(date, |l| date.min(l))
}

/// True when `date` is between `first` and `last`, where they are given.
fn allowed(
  date: Date,
  first: Option<Date>,
  last: Option<Date>,
) -> bool {
  first.is_none_or(|f| date >= f) && last.is_none_or(|l| date <= l)
}

/// A date's name in English, as assistive tools read a day:
/// "Friday 27 November 2026".
pub fn date_name(date: Date) -> String {
  format!(
    "{} {} {} {}",
    date.weekday().name(),
    date.day(),
    MONTH_NAMES[date.month() as usize - 1],
    date.year()
  )
}

/// The date a key moves the calendar's cursor to, or `None` for a key it
/// does not use.
fn date_after_key(
  at: Date,
  event: &KeyEvent,
) -> Option<Date> {
  let shift = event.modifiers.shift;
  let from_monday = i64::from(at.weekday().from_monday());
  match &event.key {
    KeyCode::Named(NamedKey::ArrowLeft) => at.add_days(-1),
    KeyCode::Named(NamedKey::ArrowRight) => at.add_days(1),
    KeyCode::Named(NamedKey::ArrowUp) => at.add_days(-7),
    KeyCode::Named(NamedKey::ArrowDown) => at.add_days(7),
    KeyCode::Named(NamedKey::PageUp) => {
      at.add_months(if shift { -12 } else { -1 })
    },
    KeyCode::Named(NamedKey::PageDown) => {
      at.add_months(if shift { 12 } else { 1 })
    },
    KeyCode::Named(NamedKey::Home) => at.add_days(-from_monday),
    KeyCode::Named(NamedKey::End) => at.add_days(6 - from_monday),
    _ => None,
  }
}

/// A month of days to choose from, a week on each line from Monday, with
/// today marked (M25).
///
/// Clicking a day, or Enter or Space on the day the keyboard is on, calls
/// `on_change` with it; the component that owns the date updates `value`.
/// The arrow keys move by a day and a week, Page Up and Page Down by a
/// month (with Shift, a year), and Home and End to the ends of the week.
/// Days before `first` or after `last` cannot be chosen. Escape calls
/// `on_close`, as when the calendar is open under a [`DateField`].
#[component]
pub fn Calendar(
  #[prop(default)] value: Option<Date>,
  #[prop(default = Callback::new(|_: Date| {}))] on_change: Callback<Date>,
  #[prop(default)] first: Option<Date>,
  #[prop(default)] last: Option<Date>,
  #[prop(into, default = "Calendar")] label: String,
  #[prop(default)] autofocus: bool,
  #[prop(default = Callback::new(|_: ()| {}))] on_close: Callback<()>,
) -> View {
  let t = theme();
  let look = t.calendar_look();
  let now = today();
  // The day the keyboard is on; the month shown is its month.
  let cursor = use_state(move || clamp(value.unwrap_or(now), first, last));
  let at = cursor.get();
  let month = at.first_of_month();
  let owner = current_component();
  let grid_target = FocusTarget::named(&format!("dovetail.calendar.{owner:?}"));
  let show_month = move |months: i32| {
    if let Some(d) = cursor.get().add_months(months) {
      cursor.set(clamp(d, first, last));
    }
  };

  let title: View = Element::new(ElementKind::Box)
    .style(Style {
      grow: 1.0,
      font_size: Some(t.text.title.size),
      font_weight: Some(t.text.title.weight),
      ..Style::default()
    })
    .role(Role::Heading)
    .children([View::text(format!(
      "{} {}",
      MONTH_NAMES[month.month() as usize - 1],
      month.year()
    ))])
    .into();
  let can_back = first.is_none_or(|f| month > f.first_of_month());
  let can_on = last.is_none_or(|l| month < l.first_of_month());
  let header: View = Element::new(ElementKind::Row)
    .style(Style {
      width: Size::Fixed(7.0 * DAY),
      align: Align::Center,
      gap: t.spacing.of(1.0),
      ..Style::default()
    })
    .children([
      ui! {
        IconButton {
          icon: dovetail_image::icons::chevron_left(),
          label: "Previous month",
          enabled: can_back,
          on_click: move |_| show_month(-1),
        }
      },
      title,
      ui! {
        IconButton {
          icon: dovetail_image::icons::chevron_right(),
          label: "Next month",
          enabled: can_on,
          on_click: move |_| show_month(1),
        }
      },
    ])
    .into();

  let cell = |content: View, colour: Color| -> View {
    Element::new(ElementKind::Row)
      .style(Style {
        width: Size::Fixed(DAY),
        height: Size::Fixed(DAY),
        align: Align::Center,
        justify: Justify::Center,
        text_color: Some(colour),
        ..Style::default()
      })
      .children([content])
      .into()
  };
  let names: View = Element::new(ElementKind::Row)
    .style(Style {
      font_size: Some(t.text.body_small.size),
      ..Style::default()
    })
    .children(
      Weekday::ALL
        .iter()
        .map(|d| cell(View::text(d.short()), look.outside)),
    )
    .into();

  // Six weeks from the Monday on or before the first of the month.
  let start = month
    .add_days(-i64::from(month.weekday().from_monday()))
    .unwrap_or(month);
  let weeks: Vec<View> = (0..6)
    .map(|week| {
      let days: Vec<View> = (0..7)
        .filter_map(|d| start.add_days(week * 7 + d))
        .map(|date| {
          let in_month = date.month() == month.month();
          let ok = allowed(date, first, last);
          let chosen = value == Some(date);
          let is_today = date == now;
          let is_cursor = date == at;
          let ink = if chosen {
            look.chosen_text
          } else if !ok {
            t.disabled_look().1
          } else if in_month {
            look.text
          } else {
            look.outside
          };
          let mut day = Element::new(ElementKind::Row)
            .style(Style {
              width: Size::Fixed(DAY),
              height: Size::Fixed(DAY),
              radius: DAY / 2.0,
              align: Align::Center,
              justify: Justify::Center,
              background: if chosen {
                look.chosen
              } else if is_cursor {
                look.hover
              } else {
                Color::TRANSPARENT
              },
              border: Border {
                width: if is_today { 1.5 } else { 0.0 },
                color: look.today,
              },
              text_color: Some(ink),
              ..Style::default()
            })
            .role(Role::GridCell)
            .label(date_name(date))
            .selected(chosen)
            .current(is_cursor)
            .children([View::text(date.day().to_string())]);
          if is_today {
            day = day.description("Today");
          }
          if ok {
            let on_change = on_change.clone();
            day = day.on_press(Callback::new(move |()| {
              cursor.set(date);
              on_change.call(date);
              grid_target.focus();
            }));
          } else {
            day = day.disabled(true);
          }
          day.into()
        })
        .collect();
      Element::new(ElementKind::Row)
        .role(Role::Row)
        .children(days)
        .into()
    })
    .collect();

  let keys = {
    let on_change = on_change.clone();
    KeyHandler::new(move |event| {
      let at = cursor.get();
      match &event.key {
        KeyCode::Named(NamedKey::Enter | NamedKey::Space) => {
          if allowed(at, first, last) {
            on_change.call(at);
          }
          return true;
        },
        KeyCode::Named(NamedKey::Escape) => {
          on_close.call(());
          return true;
        },
        _ => {},
      }
      match date_after_key(at, event) {
        Some(next) => {
          cursor.set(clamp(next, first, last));
          true
        },
        None => false,
      }
    })
  };
  let grid: View = Element::new(ElementKind::Column)
    .style(Style {
      radius: t.shapes.small,
      ..Style::default()
    })
    .role(Role::Grid)
    .label(label)
    .table_size(6, 7)
    .focusable(true)
    .autofocus(autofocus)
    .focus_target(grid_target)
    .on_key(keys)
    .children(weeks)
    .into();
  Element::new(ElementKind::Column)
    .style(Style {
      gap: t.spacing.of(1.0),
      text_color: Some(look.text),
      ..Style::default()
    })
    .children([header, names, grid])
    .into()
}

/// What a field shows below it when what was typed is refused.
fn refusal(text: &str) -> View {
  let t = theme();
  Element::new(ElementKind::Box)
    .style(Style {
      text_color: Some(t.colors.error),
      font_size: Some(t.text.body_small.size),
      ..Style::default()
    })
    .role(Role::Label)
    .children([View::text(text.to_owned())])
    .into()
}

/// A field for a date, typed as year-month-day, with a button that opens a
/// [`Calendar`] under it (M25).
///
/// A typed date is taken when Enter is pressed or focus leaves the field.
/// One that is not a date, or is before `first` or after `last`, is
/// refused: the field says why, and `value` stays as it was. Alt+Down
/// opens the calendar too. Choosing calls `on_change`; the component that
/// owns the date updates `value`.
#[component]
pub fn DateField(
  #[prop(default)] value: Option<Date>,
  #[prop(default = Callback::new(|_: Date| {}))] on_change: Callback<Date>,
  #[prop(default)] first: Option<Date>,
  #[prop(default)] last: Option<Date>,
  #[prop(into, default = "")] label: String,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  let open = use_state(|| false);
  let draft = use_state(|| None::<String>);
  let refused = use_state(|| None::<String>);
  let shown = draft
    .get()
    .unwrap_or_else(|| value.map(|d| d.to_string()).unwrap_or_default());
  let commit = {
    let on_change = on_change.clone();
    move || {
      let Some(text) = draft.get() else {
        return;
      };
      if text.trim().is_empty() {
        draft.set(None);
        refused.set(None);
        return;
      }
      match Date::parse(&text) {
        Some(d) if allowed(d, first, last) => {
          draft.set(None);
          refused.set(None);
          if Some(d) != value {
            on_change.call(d);
          }
        },
        Some(_) => refused.set(Some(match (first, last) {
          (Some(f), Some(l)) => format!("Choose a date from {f} to {l}."),
          (Some(f), None) => format!("Choose a date from {f} on."),
          (None, Some(l)) => format!("Choose a date up to {l}."),
          (None, None) => String::new(),
        })),
        None => refused.set(Some("Type a date as year-month-day.".into())),
      }
    }
  };
  let field = ui! {
    TextField {
      value: shown.clone(),
      width: 130.0,
      label: label.clone(),
      on_change: move |text: String| {
        draft.set(Some(text));
        refused.set(None);
      },
    }
  };
  let mut inside = vec![
    field,
    ui! {
      IconButton {
        icon: dovetail_image::icons::calendar(),
        label: "Choose a date",
        enabled: enabled,
        on_click: move |_| open.set(!open.get()),
      }
    },
  ];
  if open.get() && enabled {
    let on_change = on_change.clone();
    inside.push(
      Element::new(ElementKind::Box)
        .style(Style {
          layer: Some(Layer::Fill),
          ..Style::default()
        })
        .on_press(Callback::new(move |()| open.set(false)))
        .into(),
    );
    let (background, _, radius) = t.dialog_look();
    inside.push(
      Element::new(ElementKind::Box)
        .style(Style {
          layer: Some(Layer::Below),
          padding: Edges::all(t.spacing.of(2.0)),
          background,
          border: Border { width: 1.0, color: t.colors.outline_variant },
          radius,
          ..Style::default()
        })
        .on_press(Callback::new(|()| {}))
        .children([ui! {
          Calendar {
            value: value,
            first: first,
            last: last,
            label: if label.is_empty() { "Calendar".to_owned() } else { label.clone() },
            autofocus: true,
            on_change: move |d: Date| {
              open.set(false);
              draft.set(None);
              refused.set(None);
              on_change.call(d);
            },
            on_close: move |_| open.set(false),
          }
        }])
        .into(),
    );
  }
  let keys = {
    let commit = commit.clone();
    KeyHandler::new(move |event| {
      if event.is(NamedKey::Enter) {
        commit();
        return true;
      }
      if event.is(NamedKey::ArrowDown) && event.modifiers.alt {
        open.set(true);
        return true;
      }
      false
    })
  };
  let row: View = Element::new(ElementKind::Row)
    .style(Style {
      gap: t.spacing.of(1.0),
      align: Align::Center,
      ..Style::default()
    })
    .role(Role::DateInput)
    .label(label.clone())
    .text_value(value.map(|d| d.to_string()).unwrap_or_default())
    .expanded(open.get())
    .disabled(!enabled)
    .on_key(keys)
    .on_focus_within(Callback::new(move |inside: bool| {
      if !inside {
        commit();
      }
    }))
    .children(inside)
    .into();
  match refused.get() {
    Some(why) => Element::new(ElementKind::Column)
      .style(Style {
        gap: t.spacing.of(0.5),
        ..Style::default()
      })
      .children([row, refusal(&why)])
      .into(),
    None => row,
  }
}

/// One part of a [`TimeField`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
  Hour,
  Minute,
  Second,
  Half,
}

/// The parts a time field has.
fn parts(
  seconds: bool,
  twelve_hour: bool,
) -> Vec<Part> {
  let mut p = vec![Part::Hour, Part::Minute];
  if seconds {
    p.push(Part::Second);
  }
  if twelve_hour {
    p.push(Part::Half);
  }
  p
}

/// `time` with `part` moved by `by`, going round.
fn step(
  time: Time,
  part: Part,
  by: i32,
) -> Time {
  let wrap = |v: u32, n: i32| (v as i32 + by).rem_euclid(n) as u32;
  let (h, m, s) = (time.hour(), time.minute(), time.second());
  match part {
    Part::Hour => Time::new(wrap(h, 24), m, s),
    Part::Minute => Time::new(h, wrap(m, 60), s),
    Part::Second => Time::new(h, m, wrap(s, 60)),
    Part::Half => Time::new((h + 12) % 24, m, s),
  }
  .unwrap_or(time)
}

/// `time` with a typed number for `part`, or `None` when it is out of
/// range there. On the 12-hour clock the hour is 1 to 12, and keeps the
/// half of the day.
fn with_number(
  time: Time,
  part: Part,
  n: u32,
  twelve_hour: bool,
) -> Option<Time> {
  let (h, m, s) = (time.hour(), time.minute(), time.second());
  match part {
    Part::Hour if twelve_hour => {
      if !(1..=12).contains(&n) {
        return None;
      }
      Time::new(n % 12 + if h >= 12 { 12 } else { 0 }, m, s)
    },
    Part::Hour => Time::new(n, m, s),
    Part::Minute => Time::new(h, n, s),
    Part::Second => Time::new(h, m, n),
    Part::Half => None,
  }
}

/// The text of a time, as a time field reads it to assistive tools.
fn time_text(
  time: Time,
  seconds: bool,
  twelve_hour: bool,
) -> String {
  let (h12, pm) = time.hour12();
  let hour = if twelve_hour { h12 } else { time.hour() };
  let mut s = format!("{hour:02}:{:02}", time.minute());
  if seconds {
    s.push_str(&format!(":{:02}", time.second()));
  }
  if twelve_hour {
    s.push_str(if pm { " PM" } else { " AM" });
  }
  s
}

/// A field for a time of day: hours, minutes and, with `seconds`, seconds,
/// on the 24-hour clock or, with `twelve_hour`, the 12-hour one (M25).
///
/// Left and Right move between the parts, and Up and Down change the one
/// the keyboard is on, going round. Typing digits sets it; a number out of
/// range is refused, and the digit starts the part again. On the 12-hour
/// clock, A and P choose the half of the day. The buttons beside it step
/// the part with the pointer. Changing calls `on_change`; the component
/// that owns the time updates `value`.
#[component]
pub fn TimeField(
  value: Time,
  #[prop(default = Callback::new(|_: Time| {}))] on_change: Callback<Time>,
  #[prop(default)] seconds: bool,
  #[prop(default)] twelve_hour: bool,
  #[prop(into, default = "")] label: String,
  #[prop(default = true)] enabled: bool,
) -> View {
  let t = theme();
  let (background, border, focused_border, radius) = t.text_field_look();
  let list = parts(seconds, twelve_hour);
  let part = use_state(|| 0usize);
  let typed = use_state(|| None::<u32>);
  let focused = use_state(|| false);
  let at = part.get().min(list.len() - 1);
  let change = {
    let on_change = on_change.clone();
    move |new: Time| {
      if new != value {
        on_change.call(new);
      }
    }
  };
  let (h12, pm) = value.hour12();
  let texts: Vec<(Part, String)> = list
    .iter()
    .map(|&p| {
      (
        p,
        match p {
          Part::Hour if twelve_hour => format!("{h12:02}"),
          Part::Hour => format!("{:02}", value.hour()),
          Part::Minute => format!("{:02}", value.minute()),
          Part::Second => format!("{:02}", value.second()),
          Part::Half => if pm { "PM" } else { "AM" }.to_owned(),
        },
      )
    })
    .collect();
  let lit = t.colors.selection;
  let mut pieces: Vec<View> = Vec::new();
  for (i, (p, text)) in texts.into_iter().enumerate() {
    if i > 0 {
      pieces.push(View::text(if p == Part::Half { " " } else { ":" }));
    }
    pieces.push(
      Element::new(ElementKind::Box)
        .style(Style {
          padding: Edges::xy(1.0, 0.0),
          radius: 2.0,
          background: if focused.get() && i == at {
            lit
          } else {
            Color::TRANSPARENT
          },
          ..Style::default()
        })
        .on_press(Callback::new(move |()| {
          part.set(i);
          typed.set(None);
        }))
        .children([View::text(text)])
        .into(),
    );
  }
  let keys = {
    let list = list.clone();
    let change = change.clone();
    KeyHandler::new(move |event| {
      if !enabled {
        return false;
      }
      let at = part.get().min(list.len() - 1);
      let p = list[at];
      match &event.key {
        KeyCode::Named(NamedKey::ArrowLeft) => {
          part.set(at.saturating_sub(1));
          typed.set(None);
        },
        KeyCode::Named(NamedKey::ArrowRight) => {
          part.set((at + 1).min(list.len() - 1));
          typed.set(None);
        },
        KeyCode::Named(NamedKey::ArrowUp) => {
          change(step(value, p, 1));
          typed.set(None);
        },
        KeyCode::Named(NamedKey::ArrowDown) => {
          change(step(value, p, -1));
          typed.set(None);
        },
        KeyCode::Named(NamedKey::Home) => part.set(0),
        KeyCode::Named(NamedKey::End) => part.set(list.len() - 1),
        KeyCode::Character(c) if !event.modifiers.command() => {
          let c = c.to_ascii_lowercase();
          if p == Part::Half {
            let want_pm = match c.as_str() {
              "a" => false,
              "p" => true,
              _ => return true,
            };
            if want_pm != (value.hour() >= 12) {
              change(step(value, p, 1));
            }
            return true;
          }
          let Some(digit) = c.parse::<u32>().ok().filter(|_| c.len() == 1)
          else {
            return true;
          };
          let joined = typed.get().map(|first| first * 10 + digit);
          match joined.and_then(|n| with_number(value, p, n, twelve_hour)) {
            Some(new) => {
              change(new);
              typed.set(None);
              // Two digits fill a part: go on to the next.
              part.set((at + 1).min(list.len() - 1));
            },
            None => {
              // Out of range, or the first digit: it starts the part.
              if let Some(new) = with_number(value, p, digit, twelve_hour) {
                change(new);
              }
              typed.set(Some(digit));
            },
          }
        },
        _ => return false,
      }
      true
    })
  };
  let stepper = |up: bool| -> View {
    let list = list.clone();
    let change = change.clone();
    ui! {
      IconButton {
        icon: if up { dovetail_image::icons::chevron_up() } else { dovetail_image::icons::chevron_down() },
        label: if up { "Later" } else { "Earlier" },
        enabled: enabled,
        on_click: move |_| {
          let p = list[part.get().min(list.len() - 1)];
          change(step(value, p, if up { 1 } else { -1 }));
        },
      }
    }
  };
  let (off_fill, off_ink) = t.disabled_look();
  let group_label = label.clone();
  let field = Element::new(ElementKind::Row)
    .style(Style {
      padding: Edges::xy(8.0, 5.0),
      align: Align::Center,
      background: if enabled { background } else { off_fill },
      border: Border {
        width: 1.0,
        color: if focused.get() {
          focused_border
        } else {
          border
        },
      },
      radius,
      text_color: Some(if enabled {
        t.colors.on_surface
      } else {
        off_ink
      }),
      ..Style::default()
    })
    .role(Role::TimeInput)
    .label(label.clone())
    .text_value(time_text(value, seconds, twelve_hour))
    .children(pieces);
  let field: View = if enabled {
    field
      .focusable(true)
      .on_key(keys)
      .on_focus(Callback::new(move |on: bool| {
        focused.set(on);
        typed.set(None);
      }))
      .into()
  } else {
    field.disabled(true).into()
  };
  Element::new(ElementKind::Row)
    .style(Style {
      gap: t.spacing.of(0.5),
      align: Align::Center,
      ..Style::default()
    })
    .role(Role::Group)
    .label(group_label)
    .children([field, stepper(true), stepper(false)])
    .into()
}

#[cfg(test)]
mod tests {
  use dovetail::{KeyEvent, Modifiers};

  use super::*;

  fn d(
    y: i32,
    m: u32,
    day: u32,
  ) -> Date {
    Date::new(y, m, day).unwrap()
  }

  #[test]
  fn keys_move_the_calendar_by_days_weeks_and_months() {
    let at = d(2026, 1, 15); // A Thursday.
    let key = |k| KeyEvent::named(k);
    assert_eq!(
      date_after_key(at, &key(NamedKey::ArrowLeft)),
      Some(d(2026, 1, 14))
    );
    assert_eq!(
      date_after_key(at, &key(NamedKey::ArrowDown)),
      Some(d(2026, 1, 22))
    );
    assert_eq!(
      date_after_key(at, &key(NamedKey::PageUp)),
      Some(d(2025, 12, 15))
    );
    assert_eq!(
      date_after_key(at, &key(NamedKey::Home)),
      Some(d(2026, 1, 12))
    );
    assert_eq!(
      date_after_key(at, &key(NamedKey::End)),
      Some(d(2026, 1, 18))
    );
    let year = KeyEvent::named(NamedKey::PageDown)
      .with(Modifiers { shift: true, ..Modifiers::default() });
    assert_eq!(date_after_key(at, &year), Some(d(2027, 1, 15)));
    assert_eq!(date_after_key(at, &key(NamedKey::Tab)), None);
    assert_eq!(date_name(at), "Thursday 15 January 2026");
  }

  #[test]
  fn time_parts_step_round_and_refuse_numbers_out_of_range() {
    let t = Time::new(23, 59, 0).unwrap();
    assert_eq!(step(t, Part::Minute, 1), Time::new(23, 0, 0).unwrap());
    assert_eq!(step(t, Part::Hour, 1), Time::new(0, 59, 0).unwrap());
    assert_eq!(step(t, Part::Half, 1), Time::new(11, 59, 0).unwrap());
    assert_eq!(with_number(t, Part::Minute, 60, false), None);
    assert_eq!(with_number(t, Part::Hour, 24, false), None);
    assert_eq!(with_number(t, Part::Hour, 7, true), Time::new(19, 59, 0));
    assert_eq!(with_number(t, Part::Hour, 13, true), None);
    assert_eq!(time_text(t, false, true), "11:59 PM");
    assert_eq!(time_text(t, true, false), "23:59:00");
  }
}
