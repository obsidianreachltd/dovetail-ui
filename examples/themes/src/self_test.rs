//! The self-test: chooses each theme and checks, in the laid-out window:
//!
//! - the window's style (background, focus ring, selection, dark title
//!   bar) and the outermost element's background and text colour;
//! - the colours and shapes of a button, a disabled button, a switch, a
//!   text field and the extension's badge;
//! - that the overriding section's button has the error colour while the
//!   button outside it keeps the primary colour;
//! - that the system mode follows dark, light and high contrast while the
//!   window is open.
//!
//! The system's appearance is changed from inside the program, so the
//! user's settings are not touched.

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dovetail::desktop::{Application, Context, Frame, WindowId, WindowOptions};
use dovetail::layout::{Content, LayoutBox};
use dovetail::prelude::*;
use dovetail::theme::{ThemeData, choose, window_style};
use dovetail::{Role, SystemAppearance, set_system_appearance};

use crate::{Badge, CHOICE, Choice, themes};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
  Show(Choice),
  SystemDark,
  SystemLight,
  SystemHighContrast,
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

/// The element with this role whose text starts with `text`.
fn find<'a>(
  boxes: &'a [LayoutBox],
  role: Role,
  text: &str,
) -> Option<&'a dovetail::Element> {
  (0..boxes.len()).find_map(|i| {
    let e = element(&boxes[i])?;
    (e.role == role && text_of(boxes, i).starts_with(text)).then_some(e)
  })
}

/// What a theme should look like in the window: every check, with what
/// was found when one fails.
fn check(
  boxes: &[LayoutBox],
  t: &ThemeData,
) -> Result<(), String> {
  let c = &t.colors;
  let root = boxes
    .iter()
    .find_map(|b| element(b).filter(|e| e.window_style.is_some()))
    .ok_or("no window style")?;
  let want = window_style(t);
  if root.window_style != Some(want) {
    return Err(format!(
      "window style {:?}, want {want:?}",
      root.window_style
    ));
  }
  if root.style.background != c.surface
    || root.style.text_color != Some(c.on_surface)
  {
    return Err("the outermost element's colours".into());
  }
  let (fill, ink, radius, _) = t.button_look();
  let primary =
    find(boxes, Role::Button, "Primary").ok_or("no Primary button")?;
  if primary.style.background != fill
    || primary.style.text_color != Some(ink)
    || primary.style.radius != radius
  {
    return Err(format!("the Primary button: {:?}", primary.style));
  }
  let (off, off_ink) = t.disabled_look();
  let disabled =
    find(boxes, Role::Button, "Disabled").ok_or("no Disabled button")?;
  if disabled.style.background != off
    || disabled.style.text_color != Some(off_ink)
  {
    return Err("the Disabled button".into());
  }
  let switch = find(boxes, Role::Switch, "").ok_or("no switch")?;
  let track = boxes
    .iter()
    .filter_map(element)
    .find(|e| e.style.background == t.toggle_look().0)
    .is_some();
  if !track || switch.toggled != Some(true) {
    return Err("the switch's track".into());
  }
  let field = boxes
    .iter()
    .filter_map(element)
    .find(|e| e.role == Role::TextInput)
    .ok_or("no text field")?;
  let (background, border, focused, _) = t.text_field_look();
  if field.style.background != background
    || (field.style.border.color != border
      && field.style.border.color != focused)
  {
    return Err("the text field".into());
  }
  let badge = boxes
    .iter()
    .filter_map(element)
    .find(|e| e.label.as_deref() == Some("Badge"))
    .ok_or("no badge")?;
  let want = t
    .extensions
    .get::<Badge>()
    .copied()
    .ok_or("no extension")?;
  if badge.style.background != want.background
    || badge.style.text_color != Some(want.text)
  {
    return Err("the badge does not use the extension".into());
  }
  // The override: its button has the error colour; the one outside it
  // keeps the primary.
  let danger = find(boxes, Role::Button, "Delete").ok_or("no Delete button")?;
  if danger.style.background != c.error
    || danger.style.text_color != Some(c.on_error)
  {
    return Err(format!(
      "the overriding section's button: {:?}",
      danger.style.background
    ));
  }
  if c.error == c.primary {
    return Err("error and primary are the same".into());
  }
  Ok(())
}

impl Step {
  fn says(self) -> String {
    match self {
      Step::Show(c) => {
        format!("the {c:?} theme reaches the window and every control")
      },
      Step::SystemDark => {
        "the System theme turns dark when the system does".into()
      },
      Step::SystemLight => "and light again".into(),
      Step::SystemHighContrast => {
        "and takes high contrast when the system asks for it".into()
      },
    }
  }

  /// What the window should show at this step.
  fn wanted(self) -> ThemeData {
    let appearance = match self {
      Step::Show(_) | Step::SystemLight => SystemAppearance::default(),
      Step::SystemDark => SystemAppearance { dark: true, ..Default::default() },
      Step::SystemHighContrast => SystemAppearance {
        high_contrast: true,
        ..Default::default()
      },
    };
    let choice = match self {
      Step::Show(c) => c,
      _ => Choice::System,
    };
    set_system_appearance(appearance);
    let (data, dark, mode) = themes(choice);
    choose(&data, dark.as_ref(), mode)
  }
}

struct Test {
  main: Option<WindowId>,
  steps: Vec<Step>,
  at: usize,
  acted: bool,
}

static PASSED: AtomicBool = AtomicBool::new(false);

impl Application for Test {
  fn started(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    set_system_appearance(SystemAppearance::default());
    let options = WindowOptions::new("Themes self-test")
      .with_size(dovetail::desktop::LogicalSize::new(700.0, 560.0));
    match cx.open_ui_window(options, ui! { crate::App {} }) {
      Ok(id) => self.main = Some(id),
      Err(e) => {
        println!("FAIL  could not open the window: {e}");
        cx.exit();
      },
    }
    cx.wake_after(Duration::from_secs(20));
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
    let (Some(main), Some(choice)) = (self.main, CHOICE.get()) else {
      return;
    };
    let Some(&step) = self.steps.get(self.at) else {
      return;
    };
    if !self.acted {
      // Choose the theme, and set the system's appearance, for this step.
      let wanted = step.wanted();
      let _ = wanted;
      choice.set(match step {
        Step::Show(c) => c,
        _ => Choice::System,
      });
      self.acted = true;
      cx.request_redraw(main);
      return;
    }
    let wanted = step.wanted();
    match check(cx.ui_boxes(main), &wanted) {
      Ok(()) => {
        println!("PASS  {}", step.says());
        self.at += 1;
        self.acted = false;
        if self.at == self.steps.len() {
          set_system_appearance(SystemAppearance::default());
          PASSED.store(true, Ordering::Relaxed);
          println!("PASS  light, dark, custom and system themes all work");
          cx.close_window(main);
          return;
        }
      },
      Err(e) => {
        // An earlier frame; a later one shows the change. The reason is
        // kept for the time-out.
        self.last_error(e);
      },
    }
    cx.request_redraw(main);
  }

  fn woke(
    &mut self,
    cx: &mut Context<'_>,
  ) {
    let waiting = self
      .steps
      .get(self.at)
      .map_or_else(String::new, |s| s.says());
    println!("FAIL  timed out waiting for: {waiting}");
    LAST.with_borrow(|e| println!("      last: {e}"));
    set_system_appearance(SystemAppearance::default());
    cx.exit();
  }
}

thread_local! {
  static LAST: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

impl Test {
  fn last_error(
    &self,
    e: String,
  ) {
    LAST.with_borrow_mut(|l| *l = e);
  }
}

pub fn run() -> ExitCode {
  let test = Test {
    main: None,
    steps: vec![
      Step::Show(Choice::Light),
      Step::Show(Choice::Dark),
      Step::Show(Choice::Custom),
      Step::Show(Choice::System),
      Step::SystemDark,
      Step::SystemLight,
      Step::SystemHighContrast,
    ],
    at: 0,
    acted: false,
  };
  match dovetail::desktop::run(test) {
    Ok(()) if PASSED.load(Ordering::Relaxed) => ExitCode::SUCCESS,
    Ok(()) => ExitCode::FAILURE,
    Err(e) => {
      eprintln!("themes: {e}");
      ExitCode::FAILURE
    },
  }
}
