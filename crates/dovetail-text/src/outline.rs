//! Glyph outlines: lines, quadratic curves (TrueType) and cubic curves
//! (CFF), in font units with y up.

/// A point in font units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
  /// Across, to the right.
  pub x: f32,
  /// Up.
  pub y: f32,
}

impl Point {
  /// A point.
  pub const fn new(
    x: f32,
    y: f32,
  ) -> Self {
    Self { x, y }
  }

  pub(crate) fn mid(
    self,
    other: Point,
  ) -> Point {
    Point::new((self.x + other.x) / 2.0, (self.y + other.y) / 2.0)
  }
}

/// One piece of a glyph's outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
  /// A straight line.
  Line(Point, Point),
  /// A quadratic curve from the first point to the last, pulled towards
  /// the middle one.
  Quad(Point, Point, Point),
  /// A cubic curve from the first point to the last, pulled towards the
  /// two middle ones in turn.
  Cubic(Point, Point, Point, Point),
}

/// A glyph's outline: closed contours made of segments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outline {
  /// Every segment of every contour.
  pub segments: Vec<Segment>,
  /// The smallest and largest corners of the glyph's box.
  pub bounds: (Point, Point),
}

impl Outline {
  /// True when the glyph draws nothing, as for a space.
  pub fn is_empty(&self) -> bool {
    self.segments.is_empty()
  }

  /// Sets `bounds` to the box around every point, control points
  /// included, which always holds the whole outline.
  pub(crate) fn fit_bounds(&mut self) {
    let mut points = self.segments.iter().flat_map(|s| match *s {
      Segment::Line(a, b) => vec![a, b],
      Segment::Quad(a, c, b) => vec![a, c, b],
      Segment::Cubic(a, c1, c2, b) => vec![a, c1, c2, b],
    });
    let Some(first) = points.next() else {
      self.bounds = Default::default();
      return;
    };
    let (mut min, mut max) = (first, first);
    for p in points {
      min = Point::new(min.x.min(p.x), min.y.min(p.y));
      max = Point::new(max.x.max(p.x), max.y.max(p.y));
    }
    self.bounds = (min, max);
  }

  /// Adds one TrueType contour: points marked on or off the curve. Two off
  /// points in a row have an implied on point between them.
  pub(crate) fn add_contour(
    &mut self,
    points: &[(Point, bool)],
  ) {
    if points.len() < 2 {
      return;
    }
    // Start from an on-curve point, or the middle of the first two off ones.
    let start = match points.iter().position(|&(_, on)| on) {
      Some(i) => points[i].0,
      None => points[0].0.mid(points[1].0),
    };
    // Walk every point once, starting just after `first` and ending on it:
    // an on point closes the contour there, and for all-off contours the
    // last curve is closed after the loop.
    let first = points.iter().position(|&(_, on)| on).unwrap_or(0);
    let mut current = start;
    let mut control: Option<Point> = None;
    let count = points.len();
    for k in 0..count {
      let (p, on) = points[(first + 1 + k) % count];
      match (on, control) {
        (true, None) => {
          self.segments.push(Segment::Line(current, p));
          current = p;
        },
        (true, Some(c)) => {
          self.segments.push(Segment::Quad(current, c, p));
          current = p;
          control = None;
        },
        (false, None) => control = Some(p),
        (false, Some(c)) => {
          let mid = c.mid(p);
          self.segments.push(Segment::Quad(current, c, mid));
          current = mid;
          control = Some(p);
        },
      }
    }
    match control {
      Some(c) => self
        .segments
        .push(Segment::Quad(current, c, start)),
      None if current != start => {
        self.segments.push(Segment::Line(current, start))
      },
      None => {},
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn p(
    x: f32,
    y: f32,
  ) -> Point {
    Point::new(x, y)
  }

  #[test]
  fn a_square_of_on_points_closes_with_four_lines() {
    let mut o = Outline::default();
    o.add_contour(&[
      (p(0.0, 0.0), true),
      (p(10.0, 0.0), true),
      (p(10.0, 10.0), true),
      (p(0.0, 10.0), true),
    ]);
    assert_eq!(o.segments.len(), 4);
    assert_eq!(o.segments[3], Segment::Line(p(0.0, 10.0), p(0.0, 0.0)));
  }

  #[test]
  fn two_off_points_imply_an_on_point_between_them() {
    let mut o = Outline::default();
    o.add_contour(&[
      (p(0.0, 0.0), true),
      (p(0.0, 10.0), false),
      (p(10.0, 10.0), false),
      (p(10.0, 0.0), true),
    ]);
    assert_eq!(
      o.segments[0],
      Segment::Quad(p(0.0, 0.0), p(0.0, 10.0), p(5.0, 10.0))
    );
    assert_eq!(
      o.segments[1],
      Segment::Quad(p(5.0, 10.0), p(10.0, 10.0), p(10.0, 0.0))
    );
    assert_eq!(o.segments[2], Segment::Line(p(10.0, 0.0), p(0.0, 0.0)));
  }

  #[test]
  fn a_contour_of_only_off_points_is_closed_curves() {
    let mut o = Outline::default();
    o.add_contour(&[
      (p(0.0, 5.0), false),
      (p(5.0, 10.0), false),
      (p(10.0, 5.0), false),
      (p(5.0, 0.0), false),
    ]);
    assert_eq!(o.segments.len(), 4);
    assert!(
      o.segments
        .iter()
        .all(|s| matches!(s, Segment::Quad(..)))
    );
    let Segment::Quad(first, ..) = o.segments[0] else {
      unreachable!()
    };
    let Segment::Quad(.., last) = o.segments[3] else {
      unreachable!()
    };
    assert_eq!(first, last, "the contour is closed");
  }
}
