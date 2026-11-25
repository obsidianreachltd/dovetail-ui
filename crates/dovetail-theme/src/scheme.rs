//! Colour schemes: named colour roles, made from one seed colour with
//! tonal palettes in OKLCH (M14, D0019), as Flutter's
//! `ColorScheme.fromSeed` makes them with Material's HCT.

use dovetail::Color;

use crate::color::{Oklch, contrast, tone};

/// Light or dark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Brightness {
  /// Dark text on light backgrounds.
  #[default]
  Light,
  /// Light text on dark backgrounds.
  Dark,
}

/// Dovetail's own deep slate blue: the seed of the default themes.
pub const DOVETAIL_SLATE: Color = Color::hex(0x33415c);

/// The least contrast text has with its background: WCAG AA.
pub const TEXT_CONTRAST: f32 = 4.5;

/// The colours of a theme, by role. Each `on_` colour is for text and
/// icons on the colour of the same name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorScheme {
  /// Light or dark.
  pub brightness: Brightness,
  /// The main colour: filled buttons, switches that are on, focus.
  pub primary: Color,
  /// Text on `primary`.
  pub on_primary: Color,
  /// A softer primary: highlighted menu items, chosen rows.
  pub primary_container: Color,
  /// Text on `primary_container`.
  pub on_primary_container: Color,
  /// A quiet colour for less important filled things.
  pub secondary_container: Color,
  /// Text on `secondary_container`.
  pub on_secondary_container: Color,
  /// The window's background.
  pub surface: Color,
  /// Text on `surface` and the surface containers.
  pub on_surface: Color,
  /// Quieter text on `surface`, such as hints.
  pub on_surface_variant: Color,
  /// A surface a little apart from the background, such as a menu.
  pub surface_container_low: Color,
  /// Bars: menu bars, tab bars.
  pub surface_container: Color,
  /// Raised surfaces, such as dialogs.
  pub surface_container_high: Color,
  /// The most raised surfaces, such as chosen tabs and tracks.
  pub surface_container_highest: Color,
  /// Borders that must be seen, such as a text field's.
  pub outline: Color,
  /// Borders and lines that only separate.
  pub outline_variant: Color,
  /// Errors.
  pub error: Color,
  /// Text on `error`.
  pub on_error: Color,
  /// Selected text in the window in front.
  pub selection: Color,
  /// What darkens the window behind a dialog.
  pub scrim: Color,
}

/// One palette: a hue and chroma whose tones the roles take.
#[derive(Clone, Copy)]
struct Palette {
  hue: f32,
  chroma: f32,
}

impl Palette {
  fn at(
    self,
    t: f32,
  ) -> Color {
    tone(self.hue, self.chroma, t)
  }
}

/// The tone of `fg` moved away from `bg` until it has `TEXT_CONTRAST`
/// with it.
fn readable(
  palette: Palette,
  mut t: f32,
  bg: Color,
  darker: bool,
) -> Color {
  let mut c = palette.at(t);
  while contrast(c, bg) < TEXT_CONTRAST && (0.0..=100.0).contains(&t) {
    t += if darker { -1.0 } else { 1.0 };
    c = palette.at(t.clamp(0.0, 100.0));
  }
  c
}

impl ColorScheme {
  /// A scheme whose primary colour has the hue of `seed`, as Flutter's
  /// `ColorScheme.fromSeed`. Every text colour has at least WCAG AA
  /// contrast (4.5 to 1) with the background it is for.
  pub fn from_seed(
    seed: Color,
    brightness: Brightness,
  ) -> ColorScheme {
    let s = Oklch::from_color(seed);
    // A grey seed has no hue: its neutrals stay grey.
    let hue = s.h;
    let colourful = s.c.clamp(0.0, 1.0);
    // Muted seeds stay muted, and grey ones grey; very colourful ones are
    // calmed so tones keep their hue.
    let primary_chroma = colourful.min(0.19);
    let primary = Palette { hue, chroma: primary_chroma };
    let secondary = Palette {
      hue,
      chroma: (colourful * 0.3).min(0.045),
    };
    let neutral = Palette {
      hue,
      chroma: (colourful * 0.05).min(0.008),
    };
    let variant = Palette {
      hue,
      chroma: (colourful * 0.12).min(0.02),
    };
    let red = Palette { hue: 27.0, chroma: 0.19 };
    let dark = brightness == Brightness::Dark;
    // Tones of each role, light and dark, as in Material 3.
    let pick =
      |light: f32, dark_tone: f32| if dark { dark_tone } else { light };

    let surface = neutral.at(pick(98.0, 6.0));
    let primary_c = primary.at(pick(40.0, 80.0));
    let primary_container = primary.at(pick(90.0, 30.0));
    let secondary_container = secondary.at(pick(90.0, 30.0));
    let error = red.at(pick(40.0, 80.0));
    let containers = [
      neutral.at(pick(96.0, 10.0)),
      neutral.at(pick(94.0, 12.0)),
      neutral.at(pick(92.0, 17.0)),
      neutral.at(pick(90.0, 22.0)),
    ];
    // Text is checked against the most raised surface it can sit on.
    let highest = containers[3];
    let on_surface = readable(neutral, pick(10.0, 90.0), highest, !dark);
    let on_surface_variant =
      readable(variant, pick(30.0, 80.0), highest, !dark);
    // Filled primary: dark text on it in dark themes, light in light.
    let on_primary = readable(primary, pick(100.0, 20.0), primary_c, dark);
    let primary_c = if contrast(on_primary, primary_c) < TEXT_CONTRAST {
      readable(primary, pick(40.0, 80.0), on_primary, !dark)
    } else {
      primary_c
    };
    ColorScheme {
      brightness,
      primary: primary_c,
      on_primary,
      primary_container,
      on_primary_container: readable(
        primary,
        pick(10.0, 90.0),
        primary_container,
        !dark,
      ),
      secondary_container,
      on_secondary_container: readable(
        secondary,
        pick(10.0, 90.0),
        secondary_container,
        !dark,
      ),
      surface,
      on_surface,
      on_surface_variant,
      surface_container_low: containers[0],
      surface_container: containers[1],
      surface_container_high: containers[2],
      surface_container_highest: highest,
      outline: variant.at(pick(50.0, 60.0)),
      outline_variant: variant.at(pick(80.0, 30.0)),
      error,
      on_error: readable(red, pick(100.0, 20.0), error, dark),
      selection: primary.at(pick(85.0, 35.0)),
      scrim: Color {
        a: if dark { 0.5 } else { 0.25 },
        ..Color::BLACK
      },
    }
  }

  /// Like [`ColorScheme::from_seed`], but a light scheme keeps the seed
  /// itself as its primary colour, when text on it can be read, as
  /// Flutter's fidelity variant does. Dark schemes are the same as
  /// `from_seed`'s, as a dark seed would not stand out on them.
  pub fn from_seed_fidelity(
    seed: Color,
    brightness: Brightness,
  ) -> ColorScheme {
    let mut scheme = ColorScheme::from_seed(seed, brightness);
    if brightness == Brightness::Light
      && contrast(scheme.on_primary, seed) >= TEXT_CONTRAST
    {
      scheme.primary = seed;
    }
    scheme
  }

  /// The pairs of a text colour and each background it is for: what the
  /// contrast check covers.
  pub fn text_pairs(&self) -> Vec<(&'static str, Color, Color)> {
    let mut pairs = vec![
      ("on_primary on primary", self.on_primary, self.primary),
      (
        "on_primary_container on primary_container",
        self.on_primary_container,
        self.primary_container,
      ),
      (
        "on_secondary_container on secondary_container",
        self.on_secondary_container,
        self.secondary_container,
      ),
      ("on_error on error", self.on_error, self.error),
    ];
    for (name, bg) in [
      ("surface", self.surface),
      ("surface_container_low", self.surface_container_low),
      ("surface_container", self.surface_container),
      ("surface_container_high", self.surface_container_high),
      ("surface_container_highest", self.surface_container_highest),
      ("selection", self.selection),
    ] {
      pairs.push((name, self.on_surface, bg));
      if name != "selection" {
        pairs.push((name, self.on_surface_variant, bg));
      }
    }
    pairs
  }

  /// The same scheme with as much contrast as it can have, for people who
  /// ask their system for high contrast: text is black or white, and
  /// borders are as strong as text.
  pub fn high_contrast(mut self) -> ColorScheme {
    let dark = self.brightness == Brightness::Dark;
    let (ink, paper) = if dark {
      (Color::WHITE, Color::BLACK)
    } else {
      (Color::BLACK, Color::WHITE)
    };
    self.surface = paper;
    self.surface_container_low = paper;
    self.surface_container = paper;
    self.surface_container_high = paper;
    self.surface_container_highest = paper;
    self.on_surface = ink;
    self.on_surface_variant = ink;
    self.outline = ink;
    self.outline_variant = ink;
    // The primary colour, pushed until it has text contrast on the page,
    // and its text black or white.
    let p = Oklch::from_color(self.primary);
    let palette = Palette { hue: p.h, chroma: p.c };
    self.primary =
      readable(palette, if dark { 80.0 } else { 30.0 }, paper, !dark);
    self.on_primary = paper;
    self.primary_container = self.primary;
    self.on_primary_container = paper;
    self.secondary_container = paper;
    self.on_secondary_container = ink;
    self.selection = self.primary;
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn check(scheme: &ColorScheme) -> Vec<String> {
    scheme
      .text_pairs()
      .into_iter()
      .filter_map(|(name, fg, bg)| {
        let ratio = contrast(fg, bg);
        if name == "selection" && scheme.selection == scheme.primary {
          // High contrast selects with the primary, under paper text.
          return None;
        }
        (ratio < TEXT_CONTRAST).then(|| format!("{name}: {ratio:.2}"))
      })
      .collect()
  }

  /// M14 "Done when" item 5: every seed, light and dark, gives text with
  /// at least WCAG AA contrast on the backgrounds it is for.
  #[test]
  fn schemes_from_many_seeds_have_readable_text() {
    let mut seeds = vec![
      DOVETAIL_SLATE,
      Color::BLACK,
      Color::WHITE,
    ];
    for hue in (0..360).step_by(15) {
      for (l, c) in [
        (0.55, 0.2),
        (0.75, 0.15),
        (0.35, 0.1),
        (0.6, 0.04),
      ] {
        seeds.push(Oklch { l, c, h: hue as f32 }.to_color());
      }
    }
    let mut failures = Vec::new();
    for seed in &seeds {
      for b in [Brightness::Light, Brightness::Dark] {
        let scheme = ColorScheme::from_seed(*seed, b);
        for f in check(&scheme) {
          failures.push(format!("{seed:?} {b:?} {f}"));
        }
        for f in check(&ColorScheme::from_seed_fidelity(*seed, b)) {
          failures.push(format!("{seed:?} {b:?} fidelity {f}"));
        }
        for f in check(&scheme.high_contrast()) {
          failures.push(format!("{seed:?} {b:?} high contrast {f}"));
        }
      }
    }
    assert!(
      failures.is_empty(),
      "{} of {} schemes fail:\n{}",
      failures.len(),
      seeds.len() * 4,
      failures.join("\n")
    );
  }

  #[test]
  fn the_default_light_scheme_keeps_its_seed() {
    let light =
      ColorScheme::from_seed_fidelity(DOVETAIL_SLATE, Brightness::Light);
    assert_eq!(light.primary, DOVETAIL_SLATE);
    assert!(contrast(light.on_primary, light.primary) >= TEXT_CONTRAST);
  }

  #[test]
  fn the_seed_sets_the_hue_and_dark_schemes_are_dark() {
    let light = ColorScheme::from_seed(DOVETAIL_SLATE, Brightness::Light);
    let dark = ColorScheme::from_seed(DOVETAIL_SLATE, Brightness::Dark);
    let hue = |c| Oklch::from_color(c).h;
    assert!((hue(light.primary) - hue(DOVETAIL_SLATE)).abs() < 3.0);
    assert!(crate::color::luminance(light.surface) > 0.8);
    assert!(crate::color::luminance(dark.surface) < 0.05);
    assert!(crate::color::luminance(dark.on_surface) > 0.6);
  }
}
