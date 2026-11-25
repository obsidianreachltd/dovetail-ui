//! The shapes of a small PCB-like scene: a board with pads, vias and
//! traces. World units are millimetres.

/// What a shape is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
  /// A rectangle: its centre and half its size.
  Pad { centre: (f32, f32), half: (f32, f32) },
  /// A circle: its centre and radius.
  Via { centre: (f32, f32), radius: f32 },
  /// A line with round ends: its ends and half its width.
  Trace {
    from: (f32, f32),
    to: (f32, f32),
    half_width: f32,
  },
}

/// One shape.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
  /// Shown when it is selected, such as "Pad 3".
  pub name: String,
  /// Its geometry.
  pub kind: Kind,
  /// Its colour, as sRGB from 0 to 1.
  pub colour: [f32; 4],
}

const COPPER: [f32; 4] = [0.85, 0.55, 0.25, 1.0];
const VIA: [f32; 4] = [0.75, 0.75, 0.78, 1.0];
const TRACE: [f32; 4] = [0.80, 0.45, 0.20, 1.0];

impl Shape {
  /// Its bounding box: `(x0, y0, x1, y1)`.
  pub fn bounds(&self) -> (f32, f32, f32, f32) {
    match self.kind {
      Kind::Pad { centre: (x, y), half: (w, h) } => {
        (x - w, y - h, x + w, y + h)
      },
      Kind::Via { centre: (x, y), radius: r } => (x - r, y - r, x + r, y + r),
      Kind::Trace { from, to, half_width: w } => (
        from.0.min(to.0) - w,
        from.1.min(to.1) - w,
        from.0.max(to.0) + w,
        from.1.max(to.1) + w,
      ),
    }
  }

  /// How far a world point is outside the shape: negative inside.
  pub fn distance(
    &self,
    (px, py): (f32, f32),
  ) -> f32 {
    match self.kind {
      Kind::Pad { centre: (x, y), half: (w, h) } => {
        let q = ((px - x).abs() - w, (py - y).abs() - h);
        let outside = (q.0.max(0.0).powi(2) + q.1.max(0.0).powi(2)).sqrt();
        outside + q.0.max(q.1).min(0.0)
      },
      Kind::Via { centre: (x, y), radius } => {
        ((px - x).powi(2) + (py - y).powi(2)).sqrt() - radius
      },
      Kind::Trace { from: a, to: b, half_width } => {
        let (pa, ba) = ((px - a.0, py - a.1), (b.0 - a.0, b.1 - a.1));
        let h = ((pa.0 * ba.0 + pa.1 * ba.1)
          / (ba.0 * ba.0 + ba.1 * ba.1).max(1e-9))
        .clamp(0.0, 1.0);
        ((pa.0 - ba.0 * h).powi(2) + (pa.1 - ba.1 * h).powi(2)).sqrt()
          - half_width
      },
    }
  }
}

/// A scene: the board's rectangle and the shapes on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
  /// The board: `(x0, y0, x1, y1)`.
  pub board: (f32, f32, f32, f32),
  /// The shapes, drawn in order: later ones on top.
  pub shapes: Vec<Shape>,
}

impl Scene {
  /// The topmost shape at a world point, allowing `slop` world units of
  /// error round thin shapes.
  pub fn shape_at(
    &self,
    point: (f32, f32),
    slop: f32,
  ) -> Option<usize> {
    (0..self.shapes.len())
      .rev()
      .find(|&i| self.shapes[i].distance(point) <= slop)
  }

  /// The shapes whose bounds touch the world rectangle `(x0, y0, x1, y1)`.
  pub fn shapes_in(
    &self,
    (x0, y0, x1, y1): (f32, f32, f32, f32),
  ) -> Vec<usize> {
    (0..self.shapes.len())
      .filter(|&i| {
        let (a0, b0, a1, b1) = self.shapes[i].bounds();
        a0 <= x1 && a1 >= x0 && b0 <= y1 && b1 >= y0
      })
      .collect()
  }
}

/// A small board, 100 × 60 mm: two chips' pads, joined by traces, with
/// vias: 32 pads, 24 traces and 10 vias, 66 shapes.
pub fn small() -> Scene {
  let mut shapes = Vec::new();
  let mut pads = 0;
  let mut pad = |shapes: &mut Vec<Shape>, centre| {
    pads += 1;
    shapes.push(Shape {
      name: format!("Pad {pads}"),
      kind: Kind::Pad { centre, half: (1.4, 0.8) },
      colour: COPPER,
    });
  };
  // Two chips, eight pads each side.
  for (cx, cy) in [(25.0, 30.0), (75.0, 30.0)] {
    for i in 0..8 {
      let y = cy - 10.5 + i as f32 * 3.0;
      pad(&mut shapes, (cx - 6.0, y));
      pad(&mut shapes, (cx + 6.0, y));
    }
  }
  // Traces from the first chip's right pads to the second chip's left.
  for i in 0..8 {
    let y = 19.5 + i as f32 * 3.0;
    let bend = 50.0 + (i as f32 - 3.5) * 2.0;
    for (from, to) in [
      ((31.0, y), (bend, y)),
      ((bend, y), (bend, 30.0 + (y - 30.0) * 0.6)),
      ((bend, 30.0 + (y - 30.0) * 0.6), (69.0, y)),
    ] {
      shapes.push(Shape {
        name: format!(
          "Trace {}",
          shapes
            .iter()
            .filter(|s| matches!(s.kind, Kind::Trace { .. }))
            .count()
            + 1
        ),
        kind: Kind::Trace { from, to, half_width: 0.35 },
        colour: TRACE,
      });
    }
  }
  // Vias along the bottom and top.
  for i in 0..5 {
    let x = 15.0 + i as f32 * 17.5;
    for y in [6.0, 54.0] {
      shapes.push(Shape {
        name: format!(
          "Via {}",
          shapes
            .iter()
            .filter(|s| matches!(s.kind, Kind::Via { .. }))
            .count()
            + 1
        ),
        kind: Kind::Via { centre: (x, y), radius: 1.2 },
        colour: VIA,
      });
    }
  }
  Scene { board: (0.0, 0.0, 100.0, 60.0), shapes }
}

/// A large board, 1,000 × 1,000 mm, with exactly 10,000 shapes: a grid of
/// 50 × 50 cells, each with two pads, a via and a trace.
pub fn large() -> Scene {
  let mut shapes = Vec::with_capacity(10_000);
  for row in 0..50 {
    for col in 0..50 {
      let (x, y) = (10.0 + col as f32 * 20.0, 10.0 + row as f32 * 20.0);
      let n = row * 50 + col + 1;
      shapes.push(Shape {
        name: format!("Pad {}", 2 * n - 1),
        kind: Kind::Pad { centre: (x, y), half: (2.0, 1.2) },
        colour: COPPER,
      });
      shapes.push(Shape {
        name: format!("Pad {}", 2 * n),
        kind: Kind::Pad { centre: (x + 8.0, y), half: (2.0, 1.2) },
        colour: COPPER,
      });
      shapes.push(Shape {
        name: format!("Trace {n}"),
        kind: Kind::Trace {
          from: (x + 2.0, y),
          to: (x + 4.0, y + 8.0),
          half_width: 0.5,
        },
        colour: TRACE,
      });
      shapes.push(Shape {
        name: format!("Via {n}"),
        kind: Kind::Via { centre: (x + 4.0, y + 8.0), radius: 1.5 },
        colour: VIA,
      });
    }
  }
  Scene {
    board: (0.0, 0.0, 1000.0, 1000.0),
    shapes,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_scenes_have_the_shapes_described() {
    assert_eq!(small().shapes.len(), 66);
    assert_eq!(large().shapes.len(), 10_000);
  }

  #[test]
  fn shapes_are_found_by_point_and_by_box() {
    let s = small();
    let pad = s.shape_at((19.0, 19.5), 0.0).unwrap();
    assert_eq!(s.shapes[pad].name, "Pad 1");
    assert!(s.shape_at((50.0, 1.0), 0.0).is_none());
    // A box round the first chip catches its 16 pads and nothing else.
    let found = s.shapes_in((17.0, 17.0, 33.0, 43.0));
    assert!(
      found
        .iter()
        .all(|&i| s.shapes[i].name.starts_with("Pad")
          || s.shapes[i].name.starts_with("Trace"))
    );
    assert!(
      found
        .iter()
        .filter(|&&i| s.shapes[i].name.starts_with("Pad"))
        .count()
        == 16
    );
  }
}
