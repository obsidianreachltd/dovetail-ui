//! Icons: SVG outlines drawn in one colour at the size they are shown.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::path::{self, Affine};
use crate::raster;
use crate::svg::{self, Ink, Svg, SvgError};

/// How far, in pixels, a flattened curve may stray from the true one.
const TOLERANCE: f32 = 0.05;

struct Inner {
  id: u64,
  svg: Svg,
  /// The icon keeps its own colours: it paints with two or more.
  multicolour: bool,
}

/// An icon: outlines read from SVG (D0023), drawn in the colour of the text
/// around it or one it is given. Cheap to clone.
#[derive(Clone)]
pub struct Icon(Arc<Inner>);

impl std::fmt::Debug for Icon {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    write!(f, "Icon(#{})", self.0.id)
  }
}

impl PartialEq for Icon {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    self.0.id == other.0.id
  }
}

static NEXT: AtomicU64 = AtomicU64::new(1);

/// An icon drawn at one size: one coverage byte for each pixel, row by
/// row, from 0 (empty) to 255 (full).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Coverage {
  /// Width in pixels.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
  /// One byte for each pixel.
  pub alpha: Vec<u8>,
}

impl Icon {
  /// Reads an icon from SVG text in the subset D0023 describes.
  pub fn from_svg(text: &str) -> Result<Icon, SvgError> {
    let svg = svg::parse(text)?;
    // The colours it paints with, apart from the text's.
    let mut colours: Vec<[f32; 4]> = Vec::new();
    for shape in &svg.shapes {
      let fill = shape.fill.map(|_| shape.fill_ink);
      let stroke = shape.stroke.map(|_| shape.stroke_ink);
      for ink in [fill, stroke].into_iter().flatten() {
        if let Ink::Color(c) = ink
          && !colours.contains(&c)
        {
          colours.push(c);
        }
      }
    }
    let multicolour = colours.len() >= 2;
    Ok(Icon(Arc::new(Inner {
      id: NEXT.fetch_add(1, Ordering::Relaxed),
      svg,
      multicolour,
    })))
  }

  /// A number that only this icon has, for caches.
  pub fn id(&self) -> u64 {
    self.0.id
  }

  /// True when the icon keeps its own colours (D0024): it paints with two
  /// or more. Draw it with [`Icon::render_rgba`]. Otherwise it is drawn
  /// in one colour, from [`Icon::render`].
  pub fn is_multicolour(&self) -> bool {
    self.0.multicolour
  }

  /// Width over height of its `viewBox`.
  pub fn aspect(&self) -> f32 {
    let [_, _, w, h] = self.0.svg.view_box;
    w / h
  }

  /// Draws the icon in its colours to fill `width` × `height` pixels,
  /// keeping its shape and centring it, with `current` (straight sRGB, 0
  /// to 1) for the text's colour. One-colour icons are drawn in `current`
  /// throughout. Blending is in linear light.
  pub fn render_rgba(
    &self,
    width: u32,
    height: u32,
    current: [f32; 4],
  ) -> crate::Rgba {
    let n = width as usize * height as usize;
    // Premultiplied linear light.
    let mut total = vec![[0.0f32; 4]; n];
    let lin = |c: f32| {
      if c <= 0.04045 {
        c / 12.92
      } else {
        ((c + 0.055) / 1.055).powf(2.4)
      }
    };
    let multicolour = self.0.multicolour;
    self.each_layer(width, height, |cover, ink, opacity| {
      let c = match ink {
        Ink::Color(c) if multicolour => c,
        _ => current,
      };
      let rgb = [lin(c[0]), lin(c[1]), lin(c[2])];
      for (t, &a) in total.iter_mut().zip(cover) {
        let a = a * opacity * c[3];
        if a <= 0.0 {
          continue;
        }
        for k in 0..3 {
          t[k] = rgb[k] * a + t[k] * (1.0 - a);
        }
        t[3] = a + t[3] * (1.0 - a);
      }
    });
    let encode = |v: f32| {
      let v = v.clamp(0.0, 1.0);
      let s = if v <= 0.003_130_8 {
        v * 12.92
      } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
      };
      (s * 255.0 + 0.5) as u8
    };
    let mut pixels = Vec::with_capacity(n * 4);
    for t in total {
      if t[3] <= 0.0 {
        pixels.extend([0, 0, 0, 0]);
      } else {
        pixels.extend([
          encode(t[0] / t[3]),
          encode(t[1] / t[3]),
          encode(t[2] / t[3]),
          (t[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        ]);
      }
    }
    crate::Rgba { width, height, pixels }
  }

  /// Calls `layer` with each fill's and stroke's coverage, in drawing
  /// order, with its ink and opacity.
  fn each_layer(
    &self,
    width: u32,
    height: u32,
    mut layer: impl FnMut(&[f32], Ink, f32),
  ) {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 {
      return;
    }
    let [vx, vy, vw, vh] = self.0.svg.view_box;
    let scale = (width as f32 / vw).min(height as f32 / vh);
    let (ox, oy) = (
      (width as f32 - vw * scale) / 2.0,
      (height as f32 - vh * scale) / 2.0,
    );
    let to_pixels = Affine::translate(-vx, -vy)
      .then(Affine::scale(scale, scale))
      .then(Affine::translate(ox, oy));
    for shape in &self.0.svg.shapes {
      let transform = shape.transform.then(to_pixels);
      let lines = path::flatten(&shape.path, &transform, TOLERANCE);
      if let Some(rule) = shape.fill {
        let polygons: Vec<_> = lines.iter().map(|l| l.points.clone()).collect();
        let cover = raster::fill(&polygons, rule, w, h);
        layer(&cover, shape.fill_ink, shape.fill_opacity);
      }
      if let Some(mut stroke) = shape.stroke {
        stroke.width *= transform.length_scale();
        let pieces = path::stroke(&lines, &stroke, TOLERANCE);
        let cover = raster::fill(&pieces, raster::FillRule::NonZero, w, h);
        layer(&cover, shape.stroke_ink, shape.stroke_opacity);
      }
    }
  }

  /// Draws the icon to fill `width` × `height` pixels, keeping its shape
  /// and centring it, as coverage for one colour.
  pub fn render(
    &self,
    width: u32,
    height: u32,
  ) -> Coverage {
    let mut total = vec![0.0f32; width as usize * height as usize];
    // Each shape's fill and stroke are drawn over what is there.
    self.each_layer(width, height, |cover, _, opacity| {
      over(&mut total, cover, opacity);
    });
    Coverage {
      width,
      height,
      alpha: total
        .iter()
        .map(|&c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
        .collect(),
    }
  }
}

fn over(
  total: &mut [f32],
  cover: &[f32],
  opacity: f32,
) {
  for (t, &c) in total.iter_mut().zip(cover) {
    let c = c * opacity;
    *t = *t + c - *t * c;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_square_icon_fills_its_pixels() {
    let icon = Icon::from_svg(r#"<svg viewBox="0 0 4 4"><rect x="1" y="1" width="2" height="2"/></svg>"#)
      .unwrap();
    let c = icon.render(8, 8);
    assert_eq!(c.alpha[2 * 8 + 2], 255);
    assert_eq!(c.alpha[0], 0);
    assert_eq!(c.alpha.iter().filter(|&&a| a == 255).count(), 16);
  }

  #[test]
  fn a_wide_icon_is_centred() {
    let icon = Icon::from_svg(
      r#"<svg viewBox="0 0 2 1"><rect width="2" height="1"/></svg>"#,
    )
    .unwrap();
    assert_eq!(icon.aspect(), 2.0);
    let c = icon.render(4, 4);
    // Rows 1 and 2 are covered; 0 and 3 are empty.
    assert!(c.alpha[..4].iter().all(|&a| a == 0));
    assert!(c.alpha[4..12].iter().all(|&a| a == 255));
    assert!(c.alpha[12..].iter().all(|&a| a == 0));
  }

  #[test]
  fn icons_with_two_colours_keep_them() {
    let two = Icon::from_svg(
      r##"<svg viewBox="0 0 2 1"><rect width="1" height="1" fill="#f00"/><rect x="1" width="1" height="1" fill="currentColor"/></svg>"##,
    )
    .unwrap();
    assert!(
      !two.is_multicolour(),
      "one colour of its own and the text's"
    );
    let three = Icon::from_svg(
      r##"<svg viewBox="0 0 3 1"><rect width="1" height="1" fill="#f00"/><rect x="1" width="1" height="1" fill="#00f"/><rect x="2" width="1" height="1"/></svg>"##,
    )
    .unwrap();
    assert!(three.is_multicolour());
    let p = three
      .render_rgba(3, 1, [0.0, 1.0, 0.0, 1.0])
      .pixels;
    assert_eq!(
      p,
      [
        255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 255
      ]
    );
    // A one-colour icon is drawn in the text's colour, whatever it paints.
    let black = Icon::from_svg(r##"<svg viewBox="0 0 1 1"><rect width="1" height="1" fill="black"/></svg>"##).unwrap();
    assert!(!black.is_multicolour());
    assert_eq!(
      black
        .render_rgba(1, 1, [0.0, 0.0, 1.0, 1.0])
        .pixels,
      [0, 0, 255, 255]
    );
  }

  #[test]
  fn strokes_scale_with_the_icon() {
    let icon = Icon::from_svg(
      r#"<svg viewBox="0 0 10 10"><line x1="0" y1="5" x2="10" y2="5" stroke="black" stroke-width="2"/></svg>"#,
    )
    .unwrap();
    let c = icon.render(20, 20);
    // At twice the size, the line is 4 pixels thick: rows 8 to 11.
    let row = |y: usize| c.alpha[y * 20 + 10];
    assert_eq!((row(7), row(8), row(11), row(12)), (0, 255, 255, 0));
  }
}
