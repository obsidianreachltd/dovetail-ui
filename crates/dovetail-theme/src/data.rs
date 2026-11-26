//! `ThemeData`: everything a theme holds (M14, D0019), as Flutter's
//! `ThemeData`.

use std::any::{Any, TypeId};
use std::rc::Rc;
use std::time::Duration;

use dovetail::{Color, Curve, Edges, FontWeight, Transition};

use crate::color::with_alpha;
use crate::scheme::{Brightness, ColorScheme, DOVETAIL_SLATE};

/// The size and weight of one kind of text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
  /// Size in logical pixels.
  pub size: f32,
  /// Weight, from 100 to 900.
  pub weight: FontWeight,
}

impl TextStyle {
  /// Text of this size and weight.
  pub const fn new(
    size: f32,
    weight: FontWeight,
  ) -> TextStyle {
    TextStyle { size, weight }
  }
}

/// The kinds of text, by role.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextTheme {
  /// Large headings.
  pub headline: TextStyle,
  /// Titles, such as a dialog's.
  pub title: TextStyle,
  /// Most text. The outermost theme gives it to everything inside.
  pub body: TextStyle,
  /// Smaller text, such as hints and status lines.
  pub body_small: TextStyle,
  /// Text on buttons, tabs and menus.
  pub label: TextStyle,
}

impl TextTheme {
  /// Every style with its size multiplied by `scale`, as the system's text
  /// size asks (M18). The outermost [`Theme`](crate::Theme) does this with
  /// [`text_scale`](dovetail::text_scale).
  pub fn scaled(
    self,
    scale: f32,
  ) -> TextTheme {
    let s = |t: TextStyle| TextStyle { size: t.size * scale, ..t };
    TextTheme {
      headline: s(self.headline),
      title: s(self.title),
      body: s(self.body),
      body_small: s(self.body_small),
      label: s(self.label),
    }
  }
}

impl Default for TextTheme {
  fn default() -> Self {
    TextTheme {
      headline: TextStyle::new(22.0, FontWeight::SEMIBOLD),
      title: TextStyle::new(15.0, FontWeight::SEMIBOLD),
      body: TextStyle::new(14.0, FontWeight::REGULAR),
      body_small: TextStyle::new(12.0, FontWeight::REGULAR),
      // Bold, so buttons stand out.
      label: TextStyle::new(14.0, FontWeight::BOLD),
    }
  }
}

/// Corner radii.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shapes {
  /// Small things: text fields, tabs, menu items.
  pub small: f32,
  /// Buttons and menus.
  pub medium: f32,
  /// Dialogs and cards.
  pub large: f32,
}

impl Default for Shapes {
  fn default() -> Self {
    Shapes { small: 4.0, medium: 6.0, large: 8.0 }
  }
}

/// How long controls take to move (M18), as Material's and Fluent's
/// durations. With less motion set on the system, nothing moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
  /// Small changes: a switch's knob, a check box's tick, a colour.
  pub short: Duration,
  /// Larger changes: a section opening, the theme changing.
  pub medium: Duration,
  /// The curve both use.
  pub curve: Curve,
}

impl Default for Motion {
  fn default() -> Self {
    Motion {
      short: Duration::from_millis(120),
      medium: Duration::from_millis(250),
      curve: Curve::EaseOut,
    }
  }
}

impl Motion {
  /// A transition for small changes.
  pub fn short(&self) -> Transition {
    Transition::new(self.short, self.curve)
  }

  /// A transition for larger changes.
  pub fn medium(&self) -> Transition {
    Transition::new(self.medium, self.curve)
  }

  /// No motion: everything jumps, as with [`Transition::NONE`].
  pub fn none() -> Motion {
    Motion {
      short: Duration::ZERO,
      medium: Duration::ZERO,
      curve: Curve::Linear,
    }
  }
}

/// Space between and inside things, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
  /// The step every other size is a multiple of.
  pub unit: f32,
  /// Room around text inside controls, as a share of the usual: below 1
  /// is more compact, above 1 roomier.
  pub density: f32,
}

impl Default for Spacing {
  fn default() -> Self {
    Spacing { unit: 4.0, density: 1.0 }
  }
}

impl Spacing {
  /// `steps` units, scaled by the density.
  pub fn of(
    &self,
    steps: f32,
  ) -> f32 {
    (steps * self.unit * self.density).round()
  }
}

/// How buttons look, where the theme overrides the colour scheme.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ButtonTheme {
  /// The fill.
  pub background: Option<Color>,
  /// The text.
  pub foreground: Option<Color>,
  /// The corner radius.
  pub radius: Option<f32>,
  /// Room around the text.
  pub padding: Option<Edges>,
}

/// How switches look.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ToggleTheme {
  /// The track while on.
  pub on: Option<Color>,
  /// The track while off.
  pub off: Option<Color>,
  /// The knob.
  pub knob: Option<Color>,
}

/// How text fields look.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextFieldTheme {
  /// The fill.
  pub background: Option<Color>,
  /// The border while the field does not have focus.
  pub border: Option<Color>,
  /// The border while it has focus.
  pub focused_border: Option<Color>,
  /// The corner radius.
  pub radius: Option<f32>,
}

/// How menus and the menu bar look.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MenuTheme {
  /// The menu bar.
  pub bar: Option<Color>,
  /// Open menus.
  pub background: Option<Color>,
  /// The highlighted item.
  pub highlight: Option<Color>,
}

/// How tabs and docking panels look.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TabTheme {
  /// The tab bar.
  pub bar: Option<Color>,
  /// Tabs that are not chosen. The chosen tab has the surface colour.
  pub tab: Option<Color>,
  /// Lines between panels.
  pub line: Option<Color>,
}

/// How dialogs look.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DialogTheme {
  /// The box.
  pub background: Option<Color>,
  /// What darkens the window behind it.
  pub scrim: Option<Color>,
  /// The corner radius.
  pub radius: Option<f32>,
}

/// How check boxes and radio buttons look (M15).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CheckTheme {
  /// The fill of a check box that is on or mixed, and a radio button's dot.
  pub on: Option<Color>,
  /// The mark on that fill.
  pub mark: Option<Color>,
  /// The border while off.
  pub border: Option<Color>,
}

/// How sliders look (M15).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SliderTheme {
  /// The track up to the thumb.
  pub active: Option<Color>,
  /// The rest of the track.
  pub track: Option<Color>,
  /// The thumb.
  pub thumb: Option<Color>,
}

/// How tooltips look (M15).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TooltipTheme {
  /// The box.
  pub background: Option<Color>,
  /// The text.
  pub foreground: Option<Color>,
}

/// How rows in lists, trees and tables look (M16).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RowTheme {
  /// A selected row while its view has focus.
  pub selected: Option<Color>,
  /// The text of a selected row while its view has focus.
  pub selected_text: Option<Color>,
  /// A selected row while its view does not have focus.
  pub selected_idle: Option<Color>,
  /// A row under the pointer.
  pub hover: Option<Color>,
  /// The line around the row the keyboard is on.
  pub current: Option<Color>,
  /// Table headers.
  pub header: Option<Color>,
  /// Lines between headers, and where dragged rows will go.
  pub line: Option<Color>,
  /// How tall a row is, in logical pixels.
  pub height: Option<f32>,
}

/// The colours and height of rows, as [`ThemeData::row_look`] gives them
/// (M16).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowLook {
  /// A selected row while its view has focus.
  pub selected: Color,
  /// The text of a selected row while its view has focus.
  pub selected_text: Color,
  /// A selected row while its view does not have focus.
  pub selected_idle: Color,
  /// A row under the pointer.
  pub hover: Color,
  /// The line around the row the keyboard is on.
  pub current: Color,
  /// Table headers.
  pub header: Color,
  /// Lines between headers, and where dragged rows will go.
  pub line: Color,
  /// How tall a row is.
  pub height: f32,
}

/// Values an application adds to a theme for its own widgets, by type,
/// as Flutter's `ThemeExtension`.
#[derive(Clone, Default)]
pub struct Extensions(Rc<Vec<(TypeId, Rc<dyn Any>)>>);

impl std::fmt::Debug for Extensions {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    write!(f, "Extensions({})", self.0.len())
  }
}

impl PartialEq for Extensions {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    Rc::ptr_eq(&self.0, &other.0)
      || (self.0.len() == other.0.len()
        && self
          .0
          .iter()
          .zip(other.0.iter())
          .all(|(a, b)| a.0 == b.0 && Rc::ptr_eq(&a.1, &b.1)))
  }
}

impl Extensions {
  /// The extension of type `T`, if the theme has one.
  pub fn get<T: 'static>(&self) -> Option<&T> {
    self
      .0
      .iter()
      .find(|(t, _)| *t == TypeId::of::<T>())
      .and_then(|(_, v)| v.downcast_ref())
  }

  /// These extensions with `value` added, or put in place of one of the
  /// same type.
  pub fn with<T: 'static>(
    &self,
    value: T,
  ) -> Extensions {
    let mut list: Vec<(TypeId, Rc<dyn Any>)> = self
      .0
      .iter()
      .filter(|(t, _)| *t != TypeId::of::<T>())
      .cloned()
      .collect();
    list.push((TypeId::of::<T>(), Rc::new(value)));
    Extensions(Rc::new(list))
  }
}

/// Everything a theme holds: colours, text, shapes, spacing, the look of
/// each kind of control, and the application's own extensions.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeData {
  /// The colours, by role.
  pub colors: ColorScheme,
  /// The kinds of text.
  pub text: TextTheme,
  /// Corner radii.
  pub shapes: Shapes,
  /// Space between and inside things.
  pub spacing: Spacing,
  /// How long controls take to move (M18).
  pub motion: Motion,
  /// Buttons.
  pub button: ButtonTheme,
  /// Switches.
  pub toggle: ToggleTheme,
  /// Text fields.
  pub text_field: TextFieldTheme,
  /// Menus and the menu bar.
  pub menu: MenuTheme,
  /// Tabs and docking panels.
  pub tabs: TabTheme,
  /// Dialogs.
  pub dialog: DialogTheme,
  /// Check boxes and radio buttons (M15).
  pub check: CheckTheme,
  /// Sliders (M15).
  pub slider: SliderTheme,
  /// Tooltips (M15).
  pub tooltip: TooltipTheme,
  /// Rows in lists, trees and tables (M16).
  pub rows: RowTheme,
  /// The application's own values.
  pub extensions: Extensions,
}

impl Default for ThemeData {
  fn default() -> Self {
    ThemeData::light()
  }
}

impl ThemeData {
  /// A theme from a seed colour (see [`ColorScheme::from_seed`]), with the
  /// usual text, shapes and spacing.
  pub fn from_seed(
    seed: Color,
    brightness: Brightness,
  ) -> ThemeData {
    ThemeData::from_scheme(ColorScheme::from_seed(seed, brightness))
  }

  /// A theme with these colours, and the usual text, shapes and spacing.
  pub fn from_scheme(colors: ColorScheme) -> ThemeData {
    ThemeData {
      colors,
      text: TextTheme::default(),
      shapes: Shapes::default(),
      spacing: Spacing::default(),
      motion: Motion::default(),
      button: ButtonTheme::default(),
      toggle: ToggleTheme::default(),
      text_field: TextFieldTheme::default(),
      menu: MenuTheme::default(),
      tabs: TabTheme::default(),
      dialog: DialogTheme::default(),
      check: CheckTheme::default(),
      slider: SliderTheme::default(),
      tooltip: TooltipTheme::default(),
      rows: RowTheme::default(),
      extensions: Extensions::default(),
    }
  }

  /// Dovetail's light theme, from its own deep slate blue, which it keeps
  /// as the primary colour. Made once per thread.
  pub fn light() -> ThemeData {
    thread_local! {
      static LIGHT: ThemeData = ThemeData::from_scheme(ColorScheme::from_seed_fidelity(
        DOVETAIL_SLATE,
        Brightness::Light,
      ));
    }
    LIGHT.with(Clone::clone)
  }

  /// Dovetail's dark theme, from its own deep slate blue.
  pub fn dark() -> ThemeData {
    thread_local! {
      static DARK: ThemeData = ThemeData::from_seed(DOVETAIL_SLATE, Brightness::Dark);
    }
    DARK.with(Clone::clone)
  }

  /// This theme with as much contrast as it can have (see
  /// [`ColorScheme::high_contrast`]).
  pub fn high_contrast(mut self) -> ThemeData {
    self.colors = self.colors.high_contrast();
    self
  }

  /// This theme with an extension added (see [`Extensions::with`]).
  pub fn with_extension<T: 'static>(
    mut self,
    value: T,
  ) -> ThemeData {
    self.extensions = self.extensions.with(value);
    self
  }

  /// True for a dark theme.
  pub fn is_dark(&self) -> bool {
    self.colors.brightness == Brightness::Dark
  }

  // --- Resolved looks, for controls --------------------------------------

  /// A filled button's fill, text, radius and padding.
  pub fn button_look(&self) -> (Color, Color, f32, Edges) {
    let c = &self.colors;
    (
      self.button.background.unwrap_or(c.primary),
      self.button.foreground.unwrap_or(c.on_primary),
      self.button.radius.unwrap_or(self.shapes.medium),
      self
        .button
        .padding
        .unwrap_or(Edges::xy(self.spacing.of(3.5), self.spacing.of(1.75))),
    )
  }

  /// A control that cannot be used now: its fill and text.
  pub fn disabled_look(&self) -> (Color, Color) {
    let c = &self.colors;
    (
      with_alpha(c.on_surface, 0.12),
      with_alpha(c.on_surface, 0.38),
    )
  }

  /// A switch's track on, track off, and knob.
  pub fn toggle_look(&self) -> (Color, Color, Color) {
    let c = &self.colors;
    (
      self.toggle.on.unwrap_or(c.primary),
      self.toggle.off.unwrap_or(c.outline),
      self.toggle.knob.unwrap_or(c.surface),
    )
  }

  /// A text field's fill, border, border with focus, and radius.
  pub fn text_field_look(&self) -> (Color, Color, Color, f32) {
    let c = &self.colors;
    (
      self.text_field.background.unwrap_or(c.surface),
      self.text_field.border.unwrap_or(c.outline),
      self
        .text_field
        .focused_border
        .unwrap_or(c.primary),
      self
        .text_field
        .radius
        .unwrap_or(self.shapes.small),
    )
  }

  /// The menu bar, an open menu, and a highlighted item.
  pub fn menu_look(&self) -> (Color, Color, Color) {
    let c = &self.colors;
    (
      self.menu.bar.unwrap_or(c.surface_container),
      self
        .menu
        .background
        .unwrap_or(c.surface_container_low),
      self.menu.highlight.unwrap_or(c.primary_container),
    )
  }

  /// A tab bar, tabs that are not chosen, and lines between panels.
  pub fn tab_look(&self) -> (Color, Color, Color) {
    let c = &self.colors;
    (
      self.tabs.bar.unwrap_or(c.surface_container),
      self
        .tabs
        .tab
        .unwrap_or(c.surface_container_highest),
      self.tabs.line.unwrap_or(c.outline_variant),
    )
  }

  /// A check box or radio button: the fill when on, the mark on it, and
  /// the border when off.
  pub fn check_look(&self) -> (Color, Color, Color) {
    let c = &self.colors;
    (
      self.check.on.unwrap_or(c.primary),
      self.check.mark.unwrap_or(c.on_primary),
      self.check.border.unwrap_or(c.on_surface_variant),
    )
  }

  /// A slider: the track up to the thumb, the rest, and the thumb.
  pub fn slider_look(&self) -> (Color, Color, Color) {
    let c = &self.colors;
    (
      self.slider.active.unwrap_or(c.primary),
      self
        .slider
        .track
        .unwrap_or(c.surface_container_highest),
      self.slider.thumb.unwrap_or(c.primary),
    )
  }

  /// A tooltip's box, text and corner radius: dark on a light theme and
  /// light on a dark one, so it stands out from what it is over.
  pub fn tooltip_look(&self) -> (Color, Color, f32) {
    let c = &self.colors;
    (
      self.tooltip.background.unwrap_or(c.on_surface),
      self.tooltip.foreground.unwrap_or(c.surface),
      self.shapes.small,
    )
  }

  /// Rows in lists, trees and tables (M16): a selected row is filled with
  /// the primary container while its view has focus, and more quietly
  /// when it does not.
  pub fn row_look(&self) -> RowLook {
    let c = &self.colors;
    let r = &self.rows;
    RowLook {
      selected: r.selected.unwrap_or(c.primary_container),
      selected_text: r.selected_text.unwrap_or(c.on_primary_container),
      selected_idle: r
        .selected_idle
        .unwrap_or(c.surface_container_highest),
      hover: r.hover.unwrap_or(c.surface_container),
      current: r.current.unwrap_or(c.primary),
      header: r.header.unwrap_or(c.surface_container_low),
      line: r.line.unwrap_or(c.outline_variant),
      height: r.height.unwrap_or(self.spacing.of(6.5)),
    }
  }

  /// A dialog's box, the scrim behind it, and its corner radius.
  pub fn dialog_look(&self) -> (Color, Color, f32) {
    let c = &self.colors;
    (
      self
        .dialog
        .background
        .unwrap_or(c.surface_container_high),
      self.dialog.scrim.unwrap_or(c.scrim),
      self.dialog.radius.unwrap_or(self.shapes.large),
    )
  }
}
