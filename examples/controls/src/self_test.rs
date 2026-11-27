//! The self-test: uses every control in a real window, with the pointer
//! and with the keyboard, through the window's own test functions (no key
//! presses reach the system), and checks after each step:
//!
//! - check boxes, a radio group, a drop-down, a number field and a slider,
//!   each with the pointer and with keys alone;
//! - a tooltip on hover, after its pause, and on focus;
//! - dragging the list's scrollbar, a click on its track, and the wheel;
//! - a context menu from a right-click and from Shift+F10, with its
//!   shortcut shown and Paste greyed out;
//! - M25's toggle button and button group, with the pointer and keys; a
//!   date chosen from the calendar with keys and typed; a time stepped
//!   with a key; and a colour chosen with the pointer from the picker a
//!   colour button opens;
//! - the theme's colours on the controls, in light and dark.

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dovetail::desktop::{
  Application, Context, Frame, LogicalSize, WindowId, WindowOptions,
};
use dovetail::layout::{Content, LayoutBox, Rect};
use dovetail::prelude::*;
use dovetail::theme::{ThemeData, window_style};
use dovetail::{
  KeyCode, KeyEvent, Modifiers, NamedKey, PointerButton, PointerKind, Role,
};

use crate::{STATE, State};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
  Light,
  CheckBoxClick,
  CheckBoxSpace,
  RadioClick,
  RadioArrow,
  SelectOpen,
  SelectChoose,
  SelectEnter,
  SelectUp,
  SelectEnterAgain,
  NumberFocus,
  NumberArrow,
  NumberButton,
  SliderClick,
  SliderEnd,
  TooltipHover,
  TooltipLeave,
  TooltipFocus,
  ScrollDrag,
  ScrollTrack,
  ScrollWheel,
  MenuRightClick,
  MenuChoose,
  MenuFocusItem,
  MenuShiftF10,
  MenuEscape,
  PickersTab,
  ToggleClick,
  ToggleSpace,
  GroupClick,
  GroupArrow,
  CalendarOpen,
  CalendarKeys,
  DateTyped,
  TimeUp,
  ColourOpen,
  ColourHue,
  ColourEscape,
  Dark,
}

const STEPS: [Step; 39] = [
  Step::Light,
  Step::CheckBoxClick,
  Step::CheckBoxSpace,
  Step::RadioClick,
  Step::RadioArrow,
  Step::SelectOpen,
  Step::SelectChoose,
  Step::SelectEnter,
  Step::SelectUp,
  Step::SelectEnterAgain,
  Step::NumberFocus,
  Step::NumberArrow,
  Step::NumberButton,
  Step::SliderClick,
  Step::SliderEnd,
  Step::TooltipHover,
  Step::TooltipLeave,
  Step::TooltipFocus,
  Step::ScrollDrag,
  Step::ScrollTrack,
  Step::ScrollWheel,
  Step::MenuRightClick,
  Step::MenuChoose,
  Step::MenuFocusItem,
  Step::MenuShiftF10,
  Step::MenuEscape,
  Step::PickersTab,
  Step::ToggleClick,
  Step::ToggleSpace,
  Step::GroupClick,
  Step::GroupArrow,
  Step::CalendarOpen,
  Step::CalendarKeys,
  Step::DateTyped,
  Step::TimeUp,
  Step::ColourOpen,
  Step::ColourHue,
  Step::ColourEscape,
  Step::Dark,
];

impl Step {
  fn says(self) -> &'static str {
    match self {
      Step::Light => "every control shows, in the light theme's colours",
      Step::CheckBoxClick => "a click ticks a check box",
      Step::CheckBoxSpace => "Space unticks it",
      Step::RadioClick => "a click chooses a radio button",
      Step::RadioArrow => "Left moves the choice and focus to the one before",
      Step::SelectOpen => "a click opens the drop-down's list",
      Step::SelectChoose => {
        "a click on an option chooses it and closes the list"
      },
      Step::SelectEnter => "Enter opens the list again",
      Step::SelectUp => "Up moves the highlight",
      Step::SelectEnterAgain => "Enter chooses the highlighted option",
      Step::NumberFocus => "a click puts focus in the number field",
      Step::NumberArrow => "Up adds one step",
      Step::NumberButton => "the Increase button adds one more",
      Step::SliderClick => "a click on the slider's track moves it there",
      Step::SliderEnd => "End moves it to the end",
      Step::TooltipHover => "a tooltip shows after the pointer rests",
      Step::TooltipLeave => "and goes when the pointer leaves",
      Step::TooltipFocus => "Tab to the button shows its tooltip at once",
      Step::ScrollDrag => "dragging the list's scrollbar scrolls it",
      Step::ScrollTrack => "a click on the track below the thumb moves a page",
      Step::ScrollWheel => "the wheel scrolls back to the top",
      Step::MenuRightClick => {
        "a right-click opens the context menu there, with Paste greyed out"
      },
      Step::MenuChoose => "choosing Copy runs it and closes the menu",
      Step::MenuFocusItem => "a click gives focus to the list's button",
      Step::MenuShiftF10 => "Shift+F10 opens the menu below it",
      Step::MenuEscape => "Escape closes it",
      Step::PickersTab => "the Pickers tab shows M25's controls",
      Step::ToggleClick => {
        "a click presses the toggle button, in the theme's colour"
      },
      Step::ToggleSpace => "Space lets it up again",
      Step::GroupClick => "a click chooses in the button group",
      Step::GroupArrow => "Right moves the group's choice and focus",
      Step::CalendarOpen => {
        "the calendar button opens the calendar, with focus"
      },
      Step::CalendarKeys => "Right and Enter choose the next day and close it",
      Step::DateTyped => "a typed date and Enter choose it",
      Step::TimeUp => "Up adds an hour to the time",
      Step::ColourOpen => "the colour button opens the colour picker",
      Step::ColourHue => {
        "a click on the hue track a third along makes it green"
      },
      Step::ColourEscape => "Escape closes the picker",
      Step::Dark => "every control takes the dark theme's colours",
    }
  }
}

fn element(b: &LayoutBox) -> Option<&dovetail::Element> {
  match &b.content {
    Content::Element(e) => Some(e),
    _ => None,
  }
}

fn text_of(
  boxes: &[LayoutBox],
  i: usize,
) -> String {
  let mut out = String::new();
  let mut stack = vec![i];
  while let Some(i) = stack.pop() {
    match &boxes[i].content {
      Content::Text { text, .. } => out.push_str(text),
      Content::Element(_) => stack.extend(boxes[i].children.iter().rev()),
      Content::Picture { .. } => {},
    }
  }
  out
}

/// The box of the element with `role` whose label is `name`, or whose text
/// starts with it.
fn find<'a>(
  boxes: &'a [LayoutBox],
  role: Role,
  name: &str,
) -> Option<&'a LayoutBox> {
  (0..boxes.len())
    .find(|&i| {
      element(&boxes[i]).is_some_and(|e| {
        e.role == role
          && (e.label.as_deref() == Some(name)
            || text_of(boxes, i).starts_with(name))
      })
    })
    .map(|i| &boxes[i])
}

fn centre(r: Rect) -> (f32, f32) {
  (r.x + r.width / 2.0, r.y + r.height / 2.0)
}

fn state() -> State {
  STATE.get().expect("the app is built")
}

fn key(named: NamedKey) -> KeyEvent {
  KeyEvent::named(named)
}

/// What the test remembers between steps.
#[derive(Default)]
struct Memory {
  /// Where the list's first item was before any scrolling.
  first_y: Option<f32>,
  /// The thumb's top before a drag, and the offset before a track click.
  thumb_y: f32,
  offset: f32,
}

/// How far the list is scrolled: how far its first item moved up.
fn list_offset(
  boxes: &[LayoutBox],
  memory: &Memory,
) -> Option<f32> {
  let first = find(boxes, Role::Button, "First item")?;
  Some(memory.first_y? - first.rect.y)
}

/// The list's vertical scrollbar.
fn list_bar(
  cx: &Context<'_>,
  main: WindowId,
) -> Option<dovetail::layout::Scrollbar> {
  let boxes = cx.ui_boxes(main);
  let list = find(boxes, Role::Group, "Rows")?.key.clone();
  cx.scrollbars(main)
    .into_iter()
    .map(|(b, _)| b)
    .find(|b| b.key == list && b.vertical)
}

/// The colours every control should have under `t`.
fn colours(
  boxes: &[LayoutBox],
  t: &ThemeData,
) -> Result<(), String> {
  let root = boxes
    .iter()
    .find_map(|b| element(b).filter(|e| e.window_style.is_some()))
    .ok_or("no window style")?;
  if root.window_style != Some(window_style(t)) {
    return Err("the window style".into());
  }
  for (role, name) in [
    (Role::CheckBox, "Wrap lines"),
    (Role::RadioGroup, "Size"),
    (Role::ComboBox, "Fruit"),
    (Role::SpinButton, "Copies"),
    (Role::Slider, "Volume"),
    (Role::Link, "Read the help"),
    (Role::StatusBar, "Status"),
    (Role::Separator, ""),
    (Role::TabList, ""),
    (Role::Button, "More options"),
    (Role::Group, "Rows"),
  ] {
    find(boxes, role, name).ok_or(format!("no {role:?} {name:?}"))?;
  }
  let has = |colour: Color| {
    boxes
      .iter()
      .filter_map(element)
      .any(|e| e.style.background == colour)
  };
  let (active, track, _) = t.slider_look();
  if !has(active) || !has(track) {
    return Err("the slider's track".into());
  }
  let (bar, _, _) = t.menu_look();
  let status = find(boxes, Role::StatusBar, "Status").and_then(element);
  if status.map(|e| e.style.background) != Some(bar) {
    return Err("the status bar".into());
  }
  let link = find(boxes, Role::Link, "Read the help").and_then(element);
  if link.and_then(|e| e.style.text_color) != Some(t.colors.primary) {
    return Err("the link".into());
  }
  let (_, _, border) = t.check_look();
  if !boxes
    .iter()
    .filter_map(element)
    .any(|e| e.style.border.color == border && e.style.border.width > 0.0)
  {
    return Err("the check box's border".into());
  }
  Ok(())
}

struct Test {
  main: Option<WindowId>,
  at: usize,
  acted: bool,
  /// When the current step's action was done.
  since: Instant,
  memory: Memory,
  last: String,
}

static PASSED: AtomicBool = AtomicBool::new(false);

impl Test {
  /// Does the step's action.
  fn act(
    &mut self,
    cx: &mut Context<'_>,
    main: WindowId,
    step: Step,
  ) -> Result<(), String> {
    let s = state();
    let boxes = cx.ui_boxes(main);
    let at = |role, name: &str| {
      find(boxes, role, name)
        .map(|b| centre(b.rect))
        .ok_or(format!("no {role:?} {name:?} to use"))
    };
    let none = Modifiers::default();
    match step {
      Step::Light => {
        s.dark.set(false);
        self.memory.first_y =
          find(boxes, Role::Button, "First item").map(|b| b.rect.y);
      },
      Step::CheckBoxClick => {
        let (x, y) = at(Role::CheckBox, "Wrap lines")?;
        cx.click(main, x, y);
      },
      Step::CheckBoxSpace | Step::SelectEnter | Step::SelectEnterAgain => {
        let k = if step == Step::CheckBoxSpace {
          NamedKey::Space
        } else {
          NamedKey::Enter
        };
        cx.key(main, &key(k));
      },
      Step::RadioClick => {
        let (x, y) = at(Role::RadioButton, "Large")?;
        cx.click(main, x, y);
      },
      Step::RadioArrow => {
        cx.key(main, &key(NamedKey::ArrowLeft));
      },
      Step::SelectOpen => {
        let (x, y) = at(Role::ComboBox, "Fruit")?;
        cx.click(main, x, y);
      },
      Step::SelectChoose => {
        let (x, y) = at(Role::ListBoxOption, "Cherry")?;
        cx.click(main, x, y);
      },
      Step::SelectUp | Step::NumberArrow => {
        cx.key(main, &key(NamedKey::ArrowUp));
      },
      Step::NumberFocus => {
        let (x, y) = at(Role::TextInput, "Copies")?;
        cx.click(main, x, y);
      },
      Step::NumberButton => {
        let (x, y) = at(Role::Button, "Increase")?;
        cx.click(main, x, y);
      },
      Step::SliderClick => {
        let slider = find(boxes, Role::Slider, "Volume")
          .ok_or("no slider")?
          .rect;
        // A quarter of the way along the 200 pixels the thumb moves.
        let (_, y) = centre(slider);
        cx.click(main, slider.x + 8.0 + 50.0, y);
      },
      Step::SliderEnd => {
        cx.key(main, &key(NamedKey::End));
      },
      Step::TooltipHover => {
        let (x, y) = at(Role::Button, "Save")?;
        cx.pointer(main, PointerKind::Move, x, y, none);
      },
      Step::TooltipLeave => cx.pointer_leave(main),
      Step::TooltipFocus => {
        // Focus is on the slider; the Save button comes next.
        cx.key(main, &key(NamedKey::Tab));
      },
      Step::ScrollDrag => {
        let bar = list_bar(cx, main).ok_or("the list has no scrollbar")?;
        self.memory.thumb_y = bar.thumb.y;
        let (x, y) = centre(bar.thumb);
        cx.pointer(main, PointerKind::Move, x, y, none);
        cx.pointer(main, PointerKind::Down, x, y, none);
        cx.pointer(main, PointerKind::Move, x, y + 30.0, none);
        cx.pointer(main, PointerKind::Up, x, y + 30.0, none);
      },
      Step::ScrollTrack => {
        let bar = list_bar(cx, main).ok_or("the list has no scrollbar")?;
        self.memory.offset = list_offset(boxes, &self.memory).unwrap_or(0.0);
        let x = bar.track.x + bar.track.width / 2.0;
        let below = bar.thumb.y + bar.thumb.height + 4.0;
        cx.click(main, x, below);
      },
      Step::ScrollWheel => {
        let (x, y) = at(Role::Group, "Rows")?;
        cx.wheel(main, (x - 40.0, y), (0.0, -5000.0));
      },
      Step::MenuRightClick => {
        let (x, y) = at(Role::Group, "Rows")?;
        let (x, y) = (x - 40.0, y);
        cx.pointer_button(
          main,
          PointerKind::Down,
          (x, y),
          none,
          PointerButton::Secondary,
        );
        cx.pointer_button(
          main,
          PointerKind::Up,
          (x, y),
          none,
          PointerButton::Secondary,
        );
        self.memory.thumb_y = y;
        self.memory.offset = x;
      },
      Step::MenuChoose => {
        let (x, y) = at(Role::MenuItem, "Copy")?;
        cx.click(main, x, y);
      },
      Step::MenuFocusItem => {
        let (x, y) = at(Role::Button, "First item")?;
        cx.click(main, x, y);
      },
      Step::MenuShiftF10 => {
        cx.key(
          main,
          &KeyEvent {
            key: KeyCode::Named(NamedKey::F10),
            modifiers: Modifiers { shift: true, ..none },
            text: None,
            repeat: false,
          },
        );
      },
      Step::MenuEscape => {
        cx.key(main, &key(NamedKey::Escape));
      },
      Step::PickersTab => {
        let (x, y) = at(Role::Tab, "Pickers")?;
        cx.click(main, x, y);
      },
      Step::ToggleClick => {
        let (x, y) = at(Role::Button, "Bold")?;
        cx.click(main, x, y);
      },
      Step::ToggleSpace => {
        cx.key(main, &key(NamedKey::Space));
      },
      Step::GroupClick => {
        let (x, y) = at(Role::RadioButton, "Centre")?;
        cx.click(main, x, y);
      },
      Step::GroupArrow => {
        cx.key(main, &key(NamedKey::ArrowRight));
      },
      Step::CalendarOpen => {
        let (x, y) = at(Role::Button, "Choose a date")?;
        cx.click(main, x, y);
      },
      Step::CalendarKeys => {
        cx.key(main, &key(NamedKey::ArrowRight));
        cx.key(main, &key(NamedKey::Enter));
      },
      Step::DateTyped => {
        let (x, y) = at(Role::TextInput, "Due")?;
        cx.click(main, x, y);
        let mut all =
          KeyEvent::character('a').with(if cfg!(target_os = "macos") {
            Modifiers { meta: true, ..none }
          } else {
            Modifiers { control: true, ..none }
          });
        all.text = None;
        cx.key(main, &all);
        for c in "2026-03-01".chars() {
          cx.key(main, &KeyEvent::character(c));
        }
        cx.key(main, &key(NamedKey::Enter));
      },
      Step::TimeUp => {
        let (x, y) = at(Role::TimeInput, "Alarm")?;
        cx.click(main, x, y);
        cx.key(main, &key(NamedKey::ArrowUp));
      },
      Step::ColourOpen => {
        let (x, y) = at(Role::ColourWell, "Fill")?;
        cx.click(main, x, y);
      },
      Step::ColourHue => {
        let hue = find(boxes, Role::Slider, "Hue")
          .ok_or("no hue slider")?
          .rect;
        cx.click(main, hue.x + hue.width / 3.0, hue.y + hue.height / 2.0);
      },
      Step::ColourEscape => {
        cx.key(main, &key(NamedKey::Escape));
      },
      Step::Dark => {
        let (x, y) = at(Role::Button, "Dark")?;
        cx.click(main, x, y);
      },
    }
    Ok(())
  }

  /// Whether the step's result shows.
  fn check(
    &self,
    cx: &Context<'_>,
    main: WindowId,
    step: Step,
  ) -> Result<(), String> {
    let s = state();
    let boxes = cx.ui_boxes(main);
    let focused = cx.focused_element(main);
    let want = |ok: bool, what: String| if ok { Ok(()) } else { Err(what) };
    match step {
      Step::Light => colours(boxes, &ThemeData::light()),
      Step::CheckBoxClick => want(s.wrap.get(), "not ticked".into()),
      Step::CheckBoxSpace => want(!s.wrap.get(), "still ticked".into()),
      Step::RadioClick => {
        want(s.size.get() == Some(2), format!("{:?}", s.size.get()))
      },
      Step::RadioArrow => {
        let moved = s.size.get() == Some(1);
        let focus = focused.is_some_and(|e| {
          e.role == Role::RadioButton && e.toggled == Some(true)
        });
        want(
          moved && focus,
          format!("{:?}, focus moved: {focus}", s.size.get()),
        )
      },
      Step::SelectOpen | Step::SelectEnter => {
        let list = find(boxes, Role::ListBox, "Fruit").is_some();
        let open = find(boxes, Role::ComboBox, "Fruit")
          .and_then(element)
          .is_some_and(|e| e.expanded == Some(true));
        want(list && open, "the list is not open".into())
      },
      Step::SelectChoose => {
        let closed = find(boxes, Role::ListBox, "Fruit").is_none();
        want(
          s.fruit.get() == Some(3) && closed,
          format!("{:?}, closed: {closed}", s.fruit.get()),
        )
      },
      Step::SelectUp => {
        let lit = find(boxes, Role::ListBoxOption, "Blueberry")
          .and_then(element)
          .map(|e| e.style.background);
        want(
          lit.is_some_and(|c| c != Color::TRANSPARENT),
          "Blueberry is not highlighted".into(),
        )
      },
      Step::SelectEnterAgain => want(
        s.fruit.get() == Some(2)
          && find(boxes, Role::ListBox, "Fruit").is_none(),
        format!("{:?}", s.fruit.get()),
      ),
      Step::NumberFocus => want(
        focused.is_some_and(|e| e.role == Role::TextInput),
        "the field has no focus".into(),
      ),
      Step::NumberArrow => {
        want(s.copies.get() == 2.0, format!("{}", s.copies.get()))
      },
      Step::NumberButton => {
        want(s.copies.get() == 3.0, format!("{}", s.copies.get()))
      },
      Step::SliderClick => {
        want(s.volume.get() == 25.0, format!("{}", s.volume.get()))
      },
      Step::SliderEnd => {
        want(s.volume.get() == 100.0, format!("{}", s.volume.get()))
      },
      Step::TooltipHover => {
        let tip = find(boxes, Role::Tooltip, "Saves the settings");
        let t = ThemeData::light();
        let (background, _, _) = t.tooltip_look();
        let right = tip
          .and_then(element)
          .is_some_and(|e| e.style.background == background);
        let waited = self.since.elapsed() >= Duration::from_millis(400);
        want(right && waited, format!("tooltip shown: {}", tip.is_some()))
      },
      Step::TooltipLeave => want(
        find(boxes, Role::Tooltip, "").is_none(),
        "the tooltip stayed".into(),
      ),
      Step::TooltipFocus => {
        let on_save = focused.is_some_and(|e| e.role == Role::Button);
        let tip = find(boxes, Role::Tooltip, "Saves the settings").is_some();
        want(
          on_save && tip,
          format!("focus on Save: {on_save}, tooltip: {tip}"),
        )
      },
      Step::ScrollDrag => {
        let bar = list_bar(cx, main).ok_or("no scrollbar")?;
        let moved = bar.thumb.y - self.memory.thumb_y;
        let offset = list_offset(boxes, &self.memory).unwrap_or(0.0);
        want(
          (moved - 30.0).abs() < 1.0 && offset > 30.0,
          format!("thumb moved {moved}, list scrolled {offset}"),
        )
      },
      Step::ScrollTrack => {
        let bar = list_bar(cx, main).ok_or("no scrollbar")?;
        let offset = list_offset(boxes, &self.memory).unwrap_or(0.0);
        let step = offset - self.memory.offset;
        want(
          (step - bar.page).abs() < 1.0,
          format!("moved {step}, a page is {}", bar.page),
        )
      },
      Step::ScrollWheel => {
        let offset = list_offset(boxes, &self.memory).unwrap_or(-1.0);
        want(offset.abs() < 0.5, format!("scrolled {offset}"))
      },
      Step::MenuRightClick => {
        let menu = find(boxes, Role::Menu, "Row menu").ok_or("no menu")?;
        let at = (menu.rect.x, menu.rect.y);
        let paste = find(boxes, Role::MenuItem, "Paste")
          .and_then(element)
          .is_some_and(|e| e.disabled && e.on_press.is_none());
        let shortcut = boxes.iter().any(|b| {
          matches!(&b.content, Content::Text { text, .. }
            if text.ends_with('C') && text.len() > 1)
        });
        want(
          at == (self.memory.offset, self.memory.thumb_y) && paste && shortcut,
          format!(
            "menu at {at:?}, Paste greyed: {paste}, shortcut: {shortcut}"
          ),
        )
      },
      Step::MenuChoose => want(
        s.status.get() == "Copied" && find(boxes, Role::Menu, "").is_none(),
        s.status.get(),
      ),
      Step::MenuFocusItem => want(
        s.status.get() == "First item"
          && focused.is_some_and(|e| e.role == Role::Button),
        s.status.get(),
      ),
      Step::MenuShiftF10 => {
        let first = find(boxes, Role::Button, "First item").ok_or("no item")?;
        let menu = find(boxes, Role::Menu, "Row menu").ok_or("no menu")?;
        let below =
          (menu.rect.y - (first.rect.y + first.rect.height)).abs() < 1.0;
        want(below, format!("menu at {:?}", menu.rect))
      },
      Step::MenuEscape => want(
        find(boxes, Role::Menu, "").is_none(),
        "the menu stayed open".into(),
      ),
      Step::PickersTab => {
        for (role, name) in [
          (Role::Button, "Bold"),
          (Role::RadioGroup, "Alignment"),
          (Role::Group, "Styles"),
          (Role::DateInput, "Due"),
          (Role::TimeInput, "Alarm"),
          (Role::ColourWell, "Fill"),
        ] {
          find(boxes, role, name).ok_or(format!("no {role:?} {name:?}"))?;
        }
        Ok(())
      },
      Step::ToggleClick => {
        let pressed = ThemeData::light().group_look().pressed;
        let fill = find(boxes, Role::Button, "Bold")
          .and_then(element)
          .map(|e| (e.toggled, e.style.background));
        want(
          s.bold.get() && fill == Some((Some(true), pressed)),
          format!("bold {}, {fill:?}", s.bold.get()),
        )
      },
      Step::ToggleSpace => want(!s.bold.get(), "still pressed".into()),
      Step::GroupClick => {
        want(s.align.get() == 1, format!("{}", s.align.get()))
      },
      Step::GroupArrow => {
        let focus = focused.is_some_and(|e| {
          e.role == Role::RadioButton && e.toggled == Some(true)
        });
        want(
          s.align.get() == 2 && focus,
          format!("{}, focus moved: {focus}", s.align.get()),
        )
      },
      Step::CalendarOpen => want(
        focused.is_some_and(|e| e.role == Role::Grid)
          && find(boxes, Role::Heading, "January 2026").is_some(),
        "no calendar with focus".into(),
      ),
      Step::CalendarKeys => want(
        s.due.get() == dovetail::Date::new(2026, 1, 21)
          && find(boxes, Role::Grid, "").is_none(),
        format!("{:?}", s.due.get()),
      ),
      Step::DateTyped => want(
        s.due.get() == dovetail::Date::new(2026, 3, 1),
        format!("{:?}", s.due.get()),
      ),
      Step::TimeUp => {
        let t = s.alarm.get();
        want(t.hour() == 10 && t.minute() == 30, t.to_string())
      },
      Step::ColourOpen => want(
        find(boxes, Role::Slider, "Hue").is_some(),
        "no picker".into(),
      ),
      Step::ColourHue => {
        let h = dovetail::theme::color::Hsv::from_color(s.fill.get()).h;
        want((h - 120.0).abs() < 4.0, format!("hue {h}"))
      },
      Step::ColourEscape => want(
        find(boxes, Role::Slider, "Hue").is_none(),
        "the picker stayed open".into(),
      ),
      Step::Dark => colours(boxes, &ThemeData::dark()),
    }
  }
}

impl Application for Test {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let options = WindowOptions::new("Controls self-test")
      .with_size(LogicalSize::new(860.0, 680.0));
    match cx.open_ui_window(options, ui! { crate::App {} }) {
      Ok(id) => self.main = Some(id),
      Err(e) => {
        println!("FAIL  could not open the window: {e}");
        cx.exit();
      },
    }
    cx.wake_after(Duration::from_secs(60));
  }

  fn draw(
    &mut self,
    _: &mut Context<'_>,
    _: WindowId,
    _: &mut Frame,
  ) {
  }

  fn frame_presented(
    &mut self,
    cx: &mut Context<'_>,
    _: WindowId,
  ) {
    let Some(main) = self.main else {
      return;
    };
    if STATE.get().is_none() {
      return;
    }
    let Some(&step) = STEPS.get(self.at) else {
      return;
    };
    if !self.acted {
      if let Err(e) = self.act(cx, main, step) {
        println!("FAIL  {}: {e}", step.says());
        cx.exit();
        return;
      }
      self.acted = true;
      self.since = Instant::now();
      cx.request_redraw(main);
      return;
    }
    match self.check(cx, main, step) {
      Ok(()) => {
        println!("PASS  {}", step.says());
        self.at += 1;
        self.acted = false;
        if self.at == STEPS.len() {
          PASSED.store(true, Ordering::Relaxed);
          println!(
            "PASS  every control works with the pointer and the keyboard"
          );
          cx.close_window(main);
          return;
        }
      },
      Err(e) => self.last = e,
    }
    cx.request_redraw(main);
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let waiting = STEPS.get(self.at).map_or("", |s| s.says());
    println!("FAIL  timed out waiting for: {waiting}");
    println!("      last: {}", self.last);
    cx.exit();
  }
}

pub fn run() -> ExitCode {
  let test = Test {
    main: None,
    at: 0,
    acted: false,
    since: Instant::now(),
    memory: Memory::default(),
    last: String::new(),
  };
  match dovetail::desktop::run(test) {
    Ok(()) if PASSED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("controls: {e}");
      ExitCode::FAILURE
    },
  }
}
