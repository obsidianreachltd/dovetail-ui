//! Themes for Dovetail UI (M14, D0019), as in Flutter.
//!
//! - [`ThemeData`] holds a theme: a [`ColorScheme`], a [`TextTheme`],
//!   [`Shapes`], [`Spacing`], the look of each kind of control, and
//!   [`Extensions`] an application adds for its own widgets.
//! - [`ColorScheme::from_seed`] makes the colours from one seed colour,
//!   with tonal palettes in OKLCH. Text always has WCAG AA contrast.
//! - The [`Theme`] component gives a theme to everything inside it. The
//!   outermost one also fills the window with the theme's background, sets
//!   the text colour, size and weight, and tells the window its colours.
//! - [`theme()`] reads the nearest theme while a component builds, and
//!   builds the component again when it changes, as Flutter's
//!   `Theme.of(context)`.
//!
//! Every built-in control reads the theme. Without a [`Theme`], they use
//! [`ThemeData::light`].

// The macros refer to the core as `::dovetail`.
extern crate dovetail_core as dovetail;

pub mod color;
mod data;
mod scheme;

pub use data::{
  ButtonTheme, CalendarLook, CalendarTheme, CheckTheme, ColourPickerTheme,
  DialogTheme, Extensions, GroupLook, GroupTheme, MenuTheme, Motion, RowLook,
  RowTheme, Shapes, SliderTheme, Spacing, TabTheme, TextFieldTheme, TextStyle,
  TextTheme, ThemeData, ToggleTheme, TooltipTheme,
};
use std::cell::RefCell;
use std::rc::Rc;

pub use scheme::{Brightness, ColorScheme, DOVETAIL_SLATE, TEXT_CONTRAST};

use dovetail::{
  Align, Children, Element, ElementKind, Signal, Size, Style, View,
  WindowStyle, provide_context, system_appearance, text_scale, use_animated,
  use_context, use_state,
};
use dovetail_macros::component;

/// Which of a [`Theme`]'s themes to show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
  /// The dark theme while the system is dark, and the high-contrast form
  /// of the theme while the system asks for high contrast. The default.
  #[default]
  System,
  /// Always the light theme.
  Light,
  /// Always the dark theme.
  Dark,
}

/// What a [`Theme`] provides: the theme, as a signal, so the components
/// that read it are built again when it changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeSignal(pub Signal<ThemeData>);

/// The theme of the component that is building: the nearest [`Theme`]
/// around it, or [`ThemeData::light`] when there is none. The component is
/// built again when that theme changes.
///
/// With no [`Theme`], its text styles follow the system's text size, as
/// the outermost `Theme`'s do.
pub fn theme() -> ThemeData {
  match use_context::<ThemeSignal>() {
    Some(ThemeSignal(signal)) => signal.get(),
    None => {
      let mut light = ThemeData::light();
      let scale = text_scale();
      if scale != 1.0 {
        light.text = light.text.scaled(scale);
      }
      light
    },
  }
}

/// The theme `data` and `dark` give in `mode`, for the system's
/// appearance now.
pub fn choose(
  data: &ThemeData,
  dark: Option<&ThemeData>,
  mode: ThemeMode,
) -> ThemeData {
  match mode {
    ThemeMode::Light => data.clone(),
    ThemeMode::Dark => dark.unwrap_or(data).clone(),
    ThemeMode::System => {
      let system = system_appearance();
      let chosen = if system.dark {
        dark.unwrap_or(data)
      } else {
        data
      };
      if system.high_contrast {
        chosen.clone().high_contrast()
      } else {
        chosen.clone()
      }
    },
  }
}

/// How the window looks under `theme`.
pub fn window_style(theme: &ThemeData) -> WindowStyle {
  let c = &theme.colors;
  WindowStyle {
    background: c.surface,
    focus_ring: c.primary,
    selection: c.selection,
    selection_idle: c.surface_container_highest,
    dark: theme.is_dark(),
    scrollbar_thumb: c.outline,
    scrollbar_track: c.surface_container_high,
  }
}

/// Gives a theme to everything inside it, as Flutter's `Theme`.
///
/// - `data` is the theme, and `dark` the one to show while the system is
///   dark (see [`ThemeMode`]).
/// - Inside another `Theme`, it overrides the theme for its children, and
///   adds nothing to the layout: build `data` from [`theme()`] to change
///   only part of it. The system's high contrast is applied only by the
///   outermost `Theme`, so a theme built from [`theme()`] has it already.
/// - The outermost `Theme` also fills the space it is given with the
///   theme's background, gives its body text style and colour to the text
///   inside, and tells the window its colours and whether it is dark.
/// - The outermost `Theme` multiplies its text styles by the system's text
///   size, [`text_scale`](dovetail::text_scale) (M18), so the text of
///   every control grows with it. Like high contrast, it is applied once:
///   a nested theme built from [`theme()`] has it already. A size an
///   application gives text itself is not scaled.
/// - The outermost `Theme` fades its colours from one theme to the next,
///   over the theme's medium [`Motion`] time, unless less motion is set.
#[component]
pub fn Theme(
  data: ThemeData,
  #[prop(default)] dark: Option<ThemeData>,
  #[prop(default)] mode: ThemeMode,
  children: Children,
) -> View {
  // Outermost when no theme is around it on the first build, before this
  // one provides its own.
  let outermost = use_state(|| use_context::<ThemeSignal>().is_none()).get();
  let chosen = if outermost {
    let mut chosen = choose(&data, dark.as_ref(), mode);
    let scale = text_scale();
    if scale != 1.0 {
      chosen.text = chosen.text.scaled(scale);
    }
    // The colours fade from one theme to the next (M18).
    let transition = chosen.motion.medium();
    chosen.colors = use_animated(chosen.colors, transition);
    chosen
  } else {
    // A nested theme is built from the one around it, which already
    // follows the system's high contrast: it is not applied twice.
    let dark_now = match mode {
      ThemeMode::Light => false,
      ThemeMode::Dark => true,
      ThemeMode::System => system_appearance().dark,
    };
    match (&dark, dark_now) {
      (Some(d), true) => d.clone(),
      _ => data.clone(),
    }
  };
  let signal = use_state(|| chosen.clone());
  // The theme last given, kept apart from the signal so that this
  // component does not read it: while the colours fade, each build gives a
  // new theme, which must build the readers again but not this.
  let given = use_state(|| Rc::new(RefCell::new(chosen.clone()))).get();
  if *given.borrow() != chosen {
    *given.borrow_mut() = chosen.clone();
    signal.set(chosen.clone());
  }
  provide_context(ThemeSignal(signal));
  if !outermost {
    return View::fragment(children);
  }
  let c = &chosen.colors;
  Element::new(ElementKind::Stack)
    .style(Style {
      width: Size::Fill,
      height: Size::Fill,
      align: Align::Stretch,
      background: c.surface,
      text_color: Some(c.on_surface),
      font_size: Some(chosen.text.body.size),
      font_weight: Some(chosen.text.body.weight),
      ..Style::default()
    })
    .window_style(window_style(&chosen))
    .children(children)
    .into()
}
