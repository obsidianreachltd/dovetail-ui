//! Colour maths for themes: sRGB, OKLab and OKLCH, keeping colours inside
//! what a screen can show, and WCAG contrast (M14, D0019).
//!
//! OKLab is Björn Ottosson's perceptual colour space (2020). Its lightness
//! follows how light a colour looks, so tones of one hue line up.

use dovetail::Color;

/// A colour in OKLCH: lightness from 0 to 1, chroma (how colourful) from
/// 0, and hue in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
  /// Lightness, 0 (black) to 1 (white).
  pub l: f32,
  /// Chroma: 0 is grey; vivid colours are around 0.15 to 0.25.
  pub c: f32,
  /// Hue in degrees, 0 to 360.
  pub h: f32,
}

fn to_linear(c: f32) -> f32 {
  if c <= 0.04045 {
    c / 12.92
  } else {
    ((c + 0.055) / 1.055).powf(2.4)
  }
}

fn from_linear(c: f32) -> f32 {
  if c <= 0.003_130_8 {
    c * 12.92
  } else {
    1.055 * c.powf(1.0 / 2.4) - 0.055
  }
}

/// OKLab from linear sRGB.
fn oklab(
  r: f32,
  g: f32,
  b: f32,
) -> (f32, f32, f32) {
  let l = 0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b;
  let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
  let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
  let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
  (
    0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
    1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
    0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
  )
}

/// Linear sRGB from OKLab.
fn linear_from_oklab(
  l: f32,
  a: f32,
  b: f32,
) -> (f32, f32, f32) {
  let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
  let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
  let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
  let (l, m, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
  (
    4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
    -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
    -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
  )
}

impl Oklch {
  /// The OKLCH of an sRGB colour (alpha is dropped).
  pub fn from_color(c: Color) -> Oklch {
    let (l, a, b) = oklab(to_linear(c.r), to_linear(c.g), to_linear(c.b));
    let h = b.atan2(a).to_degrees().rem_euclid(360.0);
    Oklch { l, c: (a * a + b * b).sqrt(), h }
  }

  /// Linear sRGB, which may be outside 0 to 1.
  fn linear(self) -> (f32, f32, f32) {
    let (s, c) = self.h.to_radians().sin_cos();
    linear_from_oklab(self.l, self.c * c, self.c * s)
  }

  /// True when a screen can show it.
  pub fn in_gamut(self) -> bool {
    let (r, g, b) = self.linear();
    let ok = |v: f32| (-1e-4..=1.0 + 1e-4).contains(&v);
    ok(r) && ok(g) && ok(b)
  }

  /// The sRGB colour, with chroma lowered until a screen can show it, so
  /// lightness and hue stay.
  pub fn to_color(self) -> Color {
    let mut colour = self;
    if !colour.in_gamut() {
      let (mut lo, mut hi) = (0.0, self.c);
      for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if (Oklch { c: mid, ..self }).in_gamut() {
          lo = mid;
        } else {
          hi = mid;
        }
      }
      colour.c = lo;
    }
    let (r, g, b) = colour.linear();
    let channel = |v: f32| from_linear(v.clamp(0.0, 1.0));
    Color::rgb(channel(r), channel(g), channel(b))
  }
}

/// The OKLab lightness of a tone from 0 (black) to 100 (white), where tones
/// are CIE L* lightness, as Material's tonal palettes use. Greys of a tone
/// have its luminance, so contrast follows from the tones.
pub fn tone_lightness(tone: f32) -> f32 {
  let t = tone.clamp(0.0, 100.0);
  if t > 8.0 {
    (t + 16.0) / 116.0
  } else {
    (t / 903.3).cbrt()
  }
}

/// A colour of this hue and chroma at a tone from 0 to 100.
pub fn tone(
  hue: f32,
  chroma: f32,
  tone: f32,
) -> Color {
  Oklch {
    l: tone_lightness(tone),
    c: chroma,
    h: hue,
  }
  .to_color()
}

/// Relative luminance, as WCAG defines it.
pub fn luminance(c: Color) -> f32 {
  0.2126 * to_linear(c.r) + 0.7152 * to_linear(c.g) + 0.0722 * to_linear(c.b)
}

/// The WCAG contrast ratio of two colours, from 1 to 21. Text needs at
/// least 4.5 (AA).
pub fn contrast(
  a: Color,
  b: Color,
) -> f32 {
  let (x, y) = (luminance(a), luminance(b));
  (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// `c` with its alpha set to `alpha`.
pub fn with_alpha(
  c: Color,
  alpha: f32,
) -> Color {
  Color { a: alpha, ..c }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn colours_go_to_oklch_and_back() {
    for c in [
      Color::hex(0x1e66f5),
      Color::hex(0xd20f39),
      Color::hex(0x40a02b),
      Color::WHITE,
      Color::BLACK,
      Color::hex(0x808080),
    ] {
      let back = Oklch::from_color(c).to_color();
      for (a, b) in [
        (c.r, back.r),
        (c.g, back.g),
        (c.b, back.b),
      ] {
        assert!((a - b).abs() < 0.002, "{c:?} came back as {back:?}");
      }
    }
    // Known values: white is lightness 1 and grey.
    let white = Oklch::from_color(Color::WHITE);
    assert!((white.l - 1.0).abs() < 1e-3 && white.c < 1e-3);
  }

  #[test]
  fn out_of_gamut_colours_lose_chroma_not_lightness() {
    let wild = Oklch { l: 0.9, c: 0.4, h: 264.0 };
    assert!(!wild.in_gamut());
    let shown = Oklch::from_color(wild.to_color());
    assert!((shown.l - 0.9).abs() < 0.01, "{shown:?}");
    assert!(shown.c < 0.4);
  }

  #[test]
  fn contrast_matches_wcag() {
    assert!((contrast(Color::WHITE, Color::BLACK) - 21.0).abs() < 0.01);
    assert!((contrast(Color::WHITE, Color::WHITE) - 1.0).abs() < 1e-6);
    // #767676 on white is the classic 4.54:1.
    assert!((contrast(Color::hex(0x767676), Color::WHITE) - 4.54).abs() < 0.02);
  }

  #[test]
  fn tones_have_the_luminance_of_their_lightness() {
    // Tone 50 is L* 50: luminance about 0.184.
    let grey = tone(0.0, 0.0, 50.0);
    assert!(
      (luminance(grey) - 0.184).abs() < 0.005,
      "{}",
      luminance(grey)
    );
  }
}
