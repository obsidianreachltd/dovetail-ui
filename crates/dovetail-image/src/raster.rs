//! Filling polygons for icons, with the nonzero or even-odd rule.
//!
//! Each pixel row is sampled 16 times down; across, each sample row's
//! coverage is exact. Unlike the glyph rasteriser, which adds areas, this
//! counts how many times a sample is wound round, so shapes that overlap,
//! such as a stroke's pieces, are covered once.

use crate::path::P;

/// Sample rows in each pixel row.
const ROWS: usize = 16;

/// Which parts of overlapping shapes are inside.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FillRule {
  /// Inside where the shapes wind round any number of times but none.
  #[default]
  NonZero,
  /// Inside where they wind round an odd number of times.
  EvenOdd,
}

/// The coverage of `polygons`, in pixels with y down, from 0 to 1 for each
/// pixel of a `width` × `height` grid.
pub fn fill(
  polygons: &[Vec<P>],
  rule: FillRule,
  width: usize,
  height: usize,
) -> Vec<f32> {
  let mut cover = vec![0.0f32; width * height];
  if width == 0 || height == 0 {
    return cover;
  }
  // Each edge that is not flat: top, bottom, x at the top, x step per unit
  // down, and which way it winds.
  struct Edge {
    y0: f32,
    y1: f32,
    x0: f32,
    slope: f32,
    dir: i32,
  }
  let mut edges = Vec::new();
  for poly in polygons {
    let n = poly.len();
    if n < 3 {
      continue;
    }
    for i in 0..n {
      let (a, b) = (poly[i], poly[(i + 1) % n]);
      if a.y == b.y || !a.y.is_finite() || !b.y.is_finite() {
        continue;
      }
      let (top, bottom, dir) = if a.y < b.y { (a, b, 1) } else { (b, a, -1) };
      edges.push(Edge {
        y0: top.y,
        y1: bottom.y,
        x0: top.x,
        slope: (bottom.x - top.x) / (bottom.y - top.y),
        dir,
      });
    }
  }
  if edges.is_empty() {
    return cover;
  }
  let top = edges
    .iter()
    .map(|e| e.y0)
    .fold(f32::INFINITY, f32::min);
  let bottom = edges
    .iter()
    .map(|e| e.y1)
    .fold(f32::NEG_INFINITY, f32::max);
  let first = (top.floor().max(0.0) as usize).min(height);
  let last = (bottom.ceil().max(0.0) as usize).min(height);
  let weight = 1.0 / ROWS as f32;
  let mut crossings: Vec<(f32, i32)> = Vec::new();
  for py in first..last {
    let row = &mut cover[py * width..(py + 1) * width];
    for s in 0..ROWS {
      let y = py as f32 + (s as f32 + 0.5) / ROWS as f32;
      crossings.clear();
      for e in &edges {
        if y >= e.y0 && y < e.y1 {
          crossings.push((e.x0 + (y - e.y0) * e.slope, e.dir));
        }
      }
      if crossings.is_empty() {
        continue;
      }
      crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
      let mut winding = 0;
      for pair in 0..crossings.len() {
        let (x, dir) = crossings[pair];
        let inside_before = inside(winding, rule);
        winding += dir;
        let inside_after = inside(winding, rule);
        if !inside_before && inside_after {
          // A span starts here; find where it ends.
          let mut w = winding;
          let mut end = x;
          for &(x2, d2) in &crossings[pair + 1..] {
            w += d2;
            end = x2;
            if !inside(w, rule) {
              break;
            }
          }
          span(row, x, end, weight);
        }
      }
    }
  }
  for c in &mut cover {
    *c = c.min(1.0);
  }
  cover
}

fn inside(
  winding: i32,
  rule: FillRule,
) -> bool {
  match rule {
    FillRule::NonZero => winding != 0,
    FillRule::EvenOdd => winding % 2 != 0,
  }
}

/// Adds `weight` × how much of each pixel `x0..x1` covers.
fn span(
  row: &mut [f32],
  x0: f32,
  x1: f32,
  weight: f32,
) {
  let width = row.len() as f32;
  let (x0, x1) = (x0.clamp(0.0, width), x1.clamp(0.0, width));
  if x1 <= x0 {
    return;
  }
  let (p0, p1) = (x0.floor() as usize, x1.floor() as usize);
  if p0 == p1 {
    row[p0] += (x1 - x0) * weight;
    return;
  }
  row[p0] += (p0 as f32 + 1.0 - x0) * weight;
  for c in &mut row[p0 + 1..p1] {
    *c += weight;
  }
  if p1 < row.len() {
    row[p1] += (x1 - p1 as f32) * weight;
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
  ) -> Vec<P> {
    vec![
      P::new(x0, y0),
      P::new(x1, y0),
      P::new(x1, y1),
      P::new(x0, y1),
    ]
  }

  #[test]
  fn whole_pixels_are_covered_exactly() {
    let c = fill(&[square(1.0, 1.0, 3.0, 2.0)], FillRule::NonZero, 4, 3);
    assert_eq!(
      c,
      [
        0., 0., 0., 0., 0., 1., 1., 0., 0., 0., 0., 0.
      ]
    );
  }

  #[test]
  fn half_pixel_edges_are_half_covered() {
    let c = fill(&[square(0.5, 0.0, 1.5, 1.0)], FillRule::NonZero, 2, 1);
    assert_eq!(c, [0.5, 0.5]);
  }

  #[test]
  fn overlapping_shapes_cover_once() {
    // Two squares over the same half pixel: still half covered.
    let shapes = [
      square(0.0, 0.0, 0.5, 1.0),
      square(0.0, 0.0, 0.5, 1.0),
    ];
    let c = fill(&shapes, FillRule::NonZero, 1, 1);
    assert_eq!(c, [0.5]);
  }

  #[test]
  fn the_fill_rules_differ_for_a_hole_wound_the_same_way() {
    let shapes = [
      square(0.0, 0.0, 3.0, 3.0),
      square(1.0, 1.0, 2.0, 2.0),
    ];
    let nonzero = fill(&shapes, FillRule::NonZero, 3, 3);
    let even_odd = fill(&shapes, FillRule::EvenOdd, 3, 3);
    assert_eq!(nonzero[4], 1.0);
    assert_eq!(even_odd[4], 0.0);
    assert_eq!(even_odd[0], 1.0);
  }
}
