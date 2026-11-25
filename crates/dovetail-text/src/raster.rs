//! Turning outlines into anti-aliased coverage.
//!
//! The method accumulates the signed area that each edge covers in every
//! pixel, then sums along each row. It gives exact coverage for lines, with
//! no hinting, so text stays true to the font's shapes at every size.

use crate::outline::{Outline, Point, Segment};

/// A glyph drawn at one size: one coverage byte per pixel, row by row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GlyphBitmap {
  /// Width in pixels.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
  /// Pixels from the pen position to the bitmap's left edge.
  pub left: i32,
  /// Pixels from the baseline up to the bitmap's top edge.
  pub top: i32,
  /// Coverage from 0 (empty) to 255 (full).
  pub coverage: Vec<u8>,
}

/// Makes a glyph look bolder by thickening it `radius` pixels on its left
/// and right (M14): synthetic bold, for weights with no font file, as
/// browsers do. The pen position does not move.
pub fn embolden(
  bitmap: &GlyphBitmap,
  radius: f32,
) -> GlyphBitmap {
  if radius <= 0.0 || bitmap.width == 0 {
    return bitmap.clone();
  }
  let whole = radius.floor() as i32;
  let part = radius - whole as f32;
  let grow = whole + 1;
  let (w, h) = (bitmap.width as i32, bitmap.height as i32);
  let width = w + 2 * grow;
  let mut coverage = vec![0u8; (width * h) as usize];
  let at = |x: i32, y: i32| -> f32 {
    if x < 0 || x >= w {
      0.0
    } else {
      f32::from(bitmap.coverage[(y * w + x) as usize])
    }
  };
  for y in 0..h {
    for x in 0..width {
      let source = x - grow;
      let mut best = 0.0f32;
      for d in -whole..=whole {
        best = best.max(at(source + d, y));
      }
      best = best
        .max(at(source - whole - 1, y) * part)
        .max(at(source + whole + 1, y) * part);
      coverage[(y * width + x) as usize] = best.round().min(255.0) as u8;
    }
  }
  GlyphBitmap {
    width: width as u32,
    height: bitmap.height,
    left: bitmap.left - grow,
    top: bitmap.top,
    coverage,
  }
}

/// Draws `outline` with `scale` pixels per font unit. `x_offset` is the
/// pen's position inside its pixel, from 0 to 1, for placing glyphs on
/// fractions of a pixel.
pub fn rasterize(
  outline: &Outline,
  scale: f32,
  x_offset: f32,
) -> GlyphBitmap {
  if outline.is_empty() {
    return GlyphBitmap::default();
  }
  let (min, max) = outline.bounds;
  let left = (min.x * scale + x_offset).floor() as i32;
  let right = (max.x * scale + x_offset).ceil() as i32;
  let top = (max.y * scale).ceil() as i32;
  let bottom = (min.y * scale).floor() as i32;
  let width = (right - left).max(1) as usize;
  let height = (top - bottom).max(1) as usize;

  // Font units, y up, to bitmap pixels, y down.
  let to_pixels = |p: Point| {
    Point::new(
      p.x * scale + x_offset - left as f32,
      top as f32 - p.y * scale,
    )
  };
  let mut acc = Accumulator::new(width, height);
  for segment in &outline.segments {
    match *segment {
      Segment::Line(a, b) => acc.line(to_pixels(a), to_pixels(b)),
      Segment::Quad(a, c, b) => {
        acc.quad(to_pixels(a), to_pixels(c), to_pixels(b))
      },
      Segment::Cubic(a, c1, c2, b) => {
        acc.cubic(to_pixels(a), to_pixels(c1), to_pixels(c2), to_pixels(b))
      },
    }
  }
  GlyphBitmap {
    width: width as u32,
    height: height as u32,
    left,
    top,
    coverage: acc.coverage(),
  }
}

struct Accumulator {
  width: usize,
  height: usize,
  area: Vec<f32>,
}

impl Accumulator {
  fn new(
    width: usize,
    height: usize,
  ) -> Self {
    // One extra cell per row end spills into the next row, and a few at the
    // very end; the row sums still come out right.
    Self {
      width,
      height,
      area: vec![0.0; width * height + 4],
    }
  }

  fn line(
    &mut self,
    p0: Point,
    p1: Point,
  ) {
    if (p0.y - p1.y).abs() <= f32::EPSILON {
      return;
    }
    let (dir, p0, p1) = if p0.y < p1.y {
      (1.0, p0, p1)
    } else {
      (-1.0, p1, p0)
    };
    let dxdy = (p1.x - p0.x) / (p1.y - p0.y);
    let mut x = p0.x;
    if p0.y < 0.0 {
      x -= p0.y * dxdy;
    }
    let first_row = p0.y.max(0.0) as usize;
    let last_row = (p1.y.ceil() as usize).min(self.height);
    for y in first_row..last_row {
      let row = y * self.width;
      let dy = ((y + 1) as f32).min(p1.y) - (y as f32).max(p0.y);
      let x_next = x + dxdy * dy;
      let d = dy * dir;
      let (x0, x1) = if x < x_next { (x, x_next) } else { (x_next, x) };
      let x0_floor = x0.floor();
      let x0i = (x0_floor as i64).max(0) as usize;
      let x1_ceil = x1.ceil();
      let x1i = (x1_ceil as i64).max(0) as usize;
      if x1i <= x0i + 1 {
        let xmf = 0.5 * (x + x_next) - x0_floor;
        self.add(row + x0i, d - d * xmf);
        self.add(row + x0i + 1, d * xmf);
      } else {
        let s = (x1 - x0).recip();
        let x0f = x0 - x0_floor;
        let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
        let x1f = x1 - x1_ceil + 1.0;
        let am = 0.5 * s * x1f * x1f;
        self.add(row + x0i, d * a0);
        if x1i == x0i + 2 {
          self.add(row + x0i + 1, d * (1.0 - a0 - am));
        } else {
          let a1 = s * (1.5 - x0f);
          self.add(row + x0i + 1, d * (a1 - a0));
          for xi in x0i + 2..x1i - 1 {
            self.add(row + xi, d * s);
          }
          let a2 = a1 + (x1i - x0i - 3) as f32 * s;
          self.add(row + x1i - 1, d * (1.0 - a2 - am));
        }
        self.add(row + x1i, d * am);
      }
      x = x_next;
    }
  }

  fn add(
    &mut self,
    index: usize,
    value: f32,
  ) {
    if let Some(cell) = self.area.get_mut(index) {
      *cell += value;
    }
  }

  /// Splits a quadratic curve into lines, more for bigger curves.
  fn quad(
    &mut self,
    p0: Point,
    c: Point,
    p1: Point,
  ) {
    let dev = Point::new(p0.x - 2.0 * c.x + p1.x, p0.y - 2.0 * c.y + p1.y);
    let dev_sq = dev.x * dev.x + dev.y * dev.y;
    if dev_sq < 0.333 {
      self.line(p0, p1);
      return;
    }
    // A quadratic split into n lines strays from the curve by at most
    // |dev| / (8 n²). Keep that under 0.02 pixels.
    let steps = 1 + (dev_sq.sqrt() / (8.0 * 0.02)).sqrt().floor() as usize;
    let mut previous = p0;
    for i in 1..=steps {
      let t = i as f32 / steps as f32;
      let u = 1.0 - t;
      let p = Point::new(
        u * u * p0.x + 2.0 * u * t * c.x + t * t * p1.x,
        u * u * p0.y + 2.0 * u * t * c.y + t * t * p1.y,
      );
      self.line(previous, p);
      previous = p;
    }
  }

  /// Splits a cubic curve into lines, more for bigger curves.
  fn cubic(
    &mut self,
    p0: Point,
    c1: Point,
    c2: Point,
    p1: Point,
  ) {
    // The second differences bound how far the curve bends: split into n
    // lines, it strays by at most 3/4 × the larger one / n². Keep that
    // under 0.02 pixels, as for quadratics.
    let d1 = Point::new(p0.x - 2.0 * c1.x + c2.x, p0.y - 2.0 * c1.y + c2.y);
    let d2 = Point::new(c1.x - 2.0 * c2.x + p1.x, c1.y - 2.0 * c2.y + p1.y);
    let dev = (d1.x * d1.x + d1.y * d1.y)
      .max(d2.x * d2.x + d2.y * d2.y)
      .sqrt();
    let steps = 1 + (0.75 * dev / 0.02).sqrt().floor().min(500.0) as usize;
    let mut previous = p0;
    for i in 1..=steps {
      let t = i as f32 / steps as f32;
      let u = 1.0 - t;
      let (a, b, c, d) =
        (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
      let p = Point::new(
        a * p0.x + b * c1.x + c * c2.x + d * p1.x,
        a * p0.y + b * c1.y + c * c2.y + d * p1.y,
      );
      self.line(previous, p);
      previous = p;
    }
  }

  fn coverage(&self) -> Vec<u8> {
    let mut sum = 0.0f32;
    self.area[..self.width * self.height]
      .iter()
      .map(|a| {
        sum += a;
        (sum.abs().min(1.0) * 255.0).round() as u8
      })
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn square(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
  ) -> Outline {
    let mut o = Outline::default();
    let p = Point::new;
    o.add_contour(&[
      (p(x0, y0), true),
      (p(x1, y0), true),
      (p(x1, y1), true),
      (p(x0, y1), true),
    ]);
    o.bounds = (p(x0, y0), p(x1, y1));
    o
  }

  #[test]
  fn a_whole_pixel_square_is_fully_covered_and_nothing_else() {
    let g = rasterize(&square(0.0, 0.0, 4.0, 3.0), 1.0, 0.0);
    assert_eq!((g.width, g.height, g.left, g.top), (4, 3, 0, 3));
    assert!(g.coverage.iter().all(|&c| c == 255), "{:?}", g.coverage);
  }

  #[test]
  fn half_pixel_edges_are_half_covered() {
    let g = rasterize(&square(0.5, 0.0, 2.5, 1.0), 1.0, 0.0);
    assert_eq!(g.width, 3);
    assert_eq!(g.coverage, [128, 255, 128]);
  }

  #[test]
  fn winding_direction_does_not_matter() {
    let mut reversed = square(0.0, 0.0, 3.0, 3.0);
    reversed.segments.reverse();
    for s in &mut reversed.segments {
      if let Segment::Line(a, b) = *s {
        *s = Segment::Line(b, a);
      }
    }
    assert!(
      rasterize(&reversed, 1.0, 0.0)
        .coverage
        .iter()
        .all(|&c| c == 255)
    );
  }

  #[test]
  fn curves_cover_their_exact_area() {
    // Four quadratic arcs from (10, 0) to (0, 10) and round, each pulled
    // towards a corner of the 20 × 20 square. Each quarter covers the inner
    // triangle (50) plus two thirds of the outer one (2/3 × 50), so the
    // whole shape covers 4 × 83.33 = 333.3 pixels.
    let mut o = Outline::default();
    let p = Point::new;
    o.add_contour(&[
      (p(10.0, 0.0), true),
      (p(10.0, 10.0), false),
      (p(0.0, 10.0), true),
      (p(-10.0, 10.0), false),
      (p(-10.0, 0.0), true),
      (p(-10.0, -10.0), false),
      (p(0.0, -10.0), true),
      (p(10.0, -10.0), false),
    ]);
    o.bounds = (p(-10.0, -10.0), p(10.0, 10.0));
    let g = rasterize(&o, 1.0, 0.0);
    let area: f32 = g
      .coverage
      .iter()
      .map(|&c| f32::from(c) / 255.0)
      .sum();
    assert!((area - 333.33).abs() < 3.0, "area {area}");
  }
}
