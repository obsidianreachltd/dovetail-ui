//! Paths for icons: SVG path data, transforms, flattening curves into
//! lines, and turning strokes into outlines to fill.

/// A point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct P {
  /// Across, to the right.
  pub x: f32,
  /// Down.
  pub y: f32,
}

impl P {
  pub const fn new(
    x: f32,
    y: f32,
  ) -> P {
    P { x, y }
  }

  fn add(
    self,
    o: P,
  ) -> P {
    P::new(self.x + o.x, self.y + o.y)
  }

  fn sub(
    self,
    o: P,
  ) -> P {
    P::new(self.x - o.x, self.y - o.y)
  }

  fn scale(
    self,
    s: f32,
  ) -> P {
    P::new(self.x * s, self.y * s)
  }

  fn length(self) -> f32 {
    (self.x * self.x + self.y * self.y).sqrt()
  }

  fn cross(
    self,
    o: P,
  ) -> f32 {
    self.x * o.y - self.y * o.x
  }

  fn dot(
    self,
    o: P,
  ) -> f32 {
    self.x * o.x + self.y * o.y
  }

  fn lerp(
    self,
    o: P,
    t: f32,
  ) -> P {
    P::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
  }
}

/// A transform: `x' = a x + c y + e`, `y' = b x + d y + f`, as SVG has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine(pub [f32; 6]);

impl Default for Affine {
  fn default() -> Self {
    Affine::IDENTITY
  }
}

impl Affine {
  pub const IDENTITY: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

  pub fn translate(
    x: f32,
    y: f32,
  ) -> Affine {
    Affine([1.0, 0.0, 0.0, 1.0, x, y])
  }

  pub fn scale(
    x: f32,
    y: f32,
  ) -> Affine {
    Affine([x, 0.0, 0.0, y, 0.0, 0.0])
  }

  pub fn rotate(degrees: f32) -> Affine {
    let (s, c) = degrees.to_radians().sin_cos();
    Affine([c, s, -s, c, 0.0, 0.0])
  }

  /// This transform, then `then`.
  pub fn then(
    self,
    then: Affine,
  ) -> Affine {
    let [a, b, c, d, e, f] = self.0;
    let [a2, b2, c2, d2, e2, f2] = then.0;
    Affine([
      a2 * a + c2 * b,
      b2 * a + d2 * b,
      a2 * c + c2 * d,
      b2 * c + d2 * d,
      a2 * e + c2 * f + e2,
      b2 * e + d2 * f + f2,
    ])
  }

  pub fn apply(
    &self,
    p: P,
  ) -> P {
    let [a, b, c, d, e, f] = self.0;
    P::new(a * p.x + c * p.y + e, b * p.x + d * p.y + f)
  }

  /// How much lengths grow, on average: the square root of the area's
  /// growth. Used for stroke widths.
  pub fn length_scale(&self) -> f32 {
    let [a, b, c, d, _, _] = self.0;
    (a * d - b * c).abs().sqrt()
  }
}

/// One piece of a path, in absolute coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
  Move(P),
  Line(P),
  Quad(P, P),
  Cubic(P, P, P),
  Close,
}

/// A path: subpaths that each start with [`Seg::Move`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path(pub Vec<Seg>);

/// A line of points, from flattening a subpath.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Polyline {
  pub points: Vec<P>,
  pub closed: bool,
}

/// Why path data could not be read, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathError(pub usize);

/// Reads SVG path data, as in a `d` attribute.
pub fn parse(d: &str) -> Result<Path, PathError> {
  let mut r = Numbers { s: d.as_bytes(), at: 0 };
  let mut segs = Vec::new();
  let mut current = P::default();
  let mut start = P::default();
  // The last control point, for smooth curves.
  let mut last_cubic: Option<P> = None;
  let mut last_quad: Option<P> = None;
  let mut command = 0u8;
  loop {
    r.separators();
    let Some(&c) = r.s.get(r.at) else {
      break;
    };
    if c.is_ascii_alphabetic() {
      command = c;
      r.at += 1;
    } else if command == 0 {
      return Err(PathError(r.at));
    }
    // Path data starts with a move.
    if segs.is_empty() && !matches!(command, b'M' | b'm') {
      return Err(PathError(r.at));
    }
    let relative = command.is_ascii_lowercase();
    let base = if relative { current } else { P::default() };
    let point = |r: &mut Numbers<'_>| -> Result<P, PathError> {
      let x = r.number()?;
      let y = r.number()?;
      Ok(P::new(x, y).add(base))
    };
    let (mut next_cubic, mut next_quad) = (None, None);
    match command.to_ascii_uppercase() {
      b'M' => {
        current = point(&mut r)?;
        start = current;
        segs.push(Seg::Move(current));
        // More pairs after a move are lines.
        command = if relative { b'l' } else { b'L' };
      },
      b'L' => {
        current = point(&mut r)?;
        segs.push(Seg::Line(current));
      },
      b'H' => {
        let x = r.number()? + if relative { current.x } else { 0.0 };
        current = P::new(x, current.y);
        segs.push(Seg::Line(current));
      },
      b'V' => {
        let y = r.number()? + if relative { current.y } else { 0.0 };
        current = P::new(current.x, y);
        segs.push(Seg::Line(current));
      },
      b'C' => {
        let c1 = point(&mut r)?;
        let c2 = point(&mut r)?;
        current = point(&mut r)?;
        segs.push(Seg::Cubic(c1, c2, current));
        next_cubic = Some(c2);
      },
      b'S' => {
        let c1 = match last_cubic {
          Some(c) => current.add(current.sub(c)),
          None => current,
        };
        let c2 = point(&mut r)?;
        current = point(&mut r)?;
        segs.push(Seg::Cubic(c1, c2, current));
        next_cubic = Some(c2);
      },
      b'Q' => {
        let c = point(&mut r)?;
        current = point(&mut r)?;
        segs.push(Seg::Quad(c, current));
        next_quad = Some(c);
      },
      b'T' => {
        let c = match last_quad {
          Some(c) => current.add(current.sub(c)),
          None => current,
        };
        current = point(&mut r)?;
        segs.push(Seg::Quad(c, current));
        next_quad = Some(c);
      },
      b'A' => {
        let rx = r.number()?;
        let ry = r.number()?;
        let angle = r.number()?;
        let large = r.flag()?;
        let sweep = r.flag()?;
        let end = point(&mut r)?;
        arc(&mut segs, current, rx, ry, angle, large, sweep, end);
        current = end;
      },
      b'Z' => {
        segs.push(Seg::Close);
        current = start;
        // A command must follow a close.
        command = 0;
      },
      _ => return Err(PathError(r.at)),
    }
    last_cubic = next_cubic;
    last_quad = next_quad;
  }
  Ok(Path(segs))
}

struct Numbers<'a> {
  s: &'a [u8],
  at: usize,
}

impl Numbers<'_> {
  fn separators(&mut self) {
    while let Some(&c) = self.s.get(self.at) {
      if c.is_ascii_whitespace() || c == b',' {
        self.at += 1;
      } else {
        break;
      }
    }
  }

  fn number(&mut self) -> Result<f32, PathError> {
    self.separators();
    let start = self.at;
    let mut i = self.at;
    if matches!(self.s.get(i), Some(b'+' | b'-')) {
      i += 1;
    }
    let mut digits = false;
    while self.s.get(i).is_some_and(u8::is_ascii_digit) {
      i += 1;
      digits = true;
    }
    if self.s.get(i) == Some(&b'.') {
      i += 1;
      while self.s.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
        digits = true;
      }
    }
    if !digits {
      return Err(PathError(start));
    }
    if matches!(self.s.get(i), Some(b'e' | b'E')) {
      let mut j = i + 1;
      if matches!(self.s.get(j), Some(b'+' | b'-')) {
        j += 1;
      }
      if self.s.get(j).is_some_and(u8::is_ascii_digit) {
        while self.s.get(j).is_some_and(u8::is_ascii_digit) {
          j += 1;
        }
        i = j;
      }
    }
    self.at = i;
    std::str::from_utf8(&self.s[start..i])
      .ok()
      .and_then(|t| t.parse().ok())
      .ok_or(PathError(start))
  }

  /// An arc flag: one character, 0 or 1, which need not be followed by a
  /// separator.
  fn flag(&mut self) -> Result<bool, PathError> {
    self.separators();
    match self.s.get(self.at) {
      Some(b'0') => {
        self.at += 1;
        Ok(false)
      },
      Some(b'1') => {
        self.at += 1;
        Ok(true)
      },
      _ => Err(PathError(self.at)),
    }
  }
}

/// Adds an elliptical arc as cubic curves, as SVG's implementation notes
/// describe.
#[allow(clippy::too_many_arguments)]
fn arc(
  segs: &mut Vec<Seg>,
  from: P,
  rx: f32,
  ry: f32,
  angle: f32,
  large: bool,
  sweep: bool,
  to: P,
) {
  if from == to {
    return;
  }
  let (mut rx, mut ry) = (rx.abs(), ry.abs());
  if rx == 0.0 || ry == 0.0 {
    segs.push(Seg::Line(to));
    return;
  }
  let (sin, cos) = angle.to_radians().sin_cos();
  // The ends, moved so the middle between them is the origin, and turned.
  let dx = (from.x - to.x) / 2.0;
  let dy = (from.y - to.y) / 2.0;
  let x1 = cos * dx + sin * dy;
  let y1 = -sin * dx + cos * dy;
  // Radii too small to reach are made larger.
  let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
  if lambda > 1.0 {
    let s = lambda.sqrt();
    rx *= s;
    ry *= s;
  }
  let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
  let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
  let mut k = (num / den).max(0.0).sqrt();
  if large == sweep {
    k = -k;
  }
  let cx1 = k * rx * y1 / ry;
  let cy1 = -k * ry * x1 / rx;
  let cx = cos * cx1 - sin * cy1 + (from.x + to.x) / 2.0;
  let cy = sin * cx1 + cos * cy1 + (from.y + to.y) / 2.0;
  let angle_of = |ux: f32, uy: f32| uy.atan2(ux);
  let theta = angle_of((x1 - cx1) / rx, (y1 - cy1) / ry);
  let mut delta = angle_of((-x1 - cx1) / rx, (-y1 - cy1) / ry) - theta;
  let tau = std::f32::consts::TAU;
  if sweep && delta < 0.0 {
    delta += tau;
  } else if !sweep && delta > 0.0 {
    delta -= tau;
  }
  // At most a quarter turn for each cubic.
  let pieces = (delta.abs() / (tau / 4.0)).ceil().max(1.0) as usize;
  let step = delta / pieces as f32;
  let t = 4.0 / 3.0 * (step / 4.0).tan();
  let point_at = |a: f32| {
    let (s, c) = a.sin_cos();
    (
      P::new(
        cx + rx * c * cos - ry * s * sin,
        cy + rx * c * sin + ry * s * cos,
      ),
      // The tangent there.
      P::new(-rx * s * cos - ry * c * sin, -rx * s * sin + ry * c * cos),
    )
  };
  let mut a = theta;
  let (mut p0, mut d0) = point_at(a);
  for i in 0..pieces {
    a += step;
    let (mut p1, d1) = point_at(a);
    if i == pieces - 1 {
      p1 = to;
    }
    segs.push(Seg::Cubic(p0.add(d0.scale(t)), p1.sub(d1.scale(t)), p1));
    p0 = p1;
    d0 = d1;
  }
}

/// Turns a path into lines, after `transform`, with curves split until
/// they stray less than `tolerance` from the true curve.
pub fn flatten(
  path: &Path,
  transform: &Affine,
  tolerance: f32,
) -> Vec<Polyline> {
  let mut out: Vec<Polyline> = Vec::new();
  let mut current = Polyline::default();
  let mut at = P::default();
  let mut start = P::default();
  let finish = |line: &mut Polyline, out: &mut Vec<Polyline>| {
    if !line.points.is_empty() {
      out.push(std::mem::take(line));
    }
  };
  for seg in &path.0 {
    match *seg {
      Seg::Move(p) => {
        finish(&mut current, &mut out);
        at = transform.apply(p);
        start = at;
        current.points.push(at);
      },
      Seg::Line(p) => {
        if current.points.is_empty() {
          current.points.push(at);
        }
        at = transform.apply(p);
        current.points.push(at);
      },
      Seg::Quad(c, p) => {
        if current.points.is_empty() {
          current.points.push(at);
        }
        let (c, p) = (transform.apply(c), transform.apply(p));
        // A quadratic is a cubic with these controls.
        let c1 = at.lerp(c, 2.0 / 3.0);
        let c2 = p.lerp(c, 2.0 / 3.0);
        cubic(&mut current.points, at, c1, c2, p, tolerance);
        at = p;
      },
      Seg::Cubic(c1, c2, p) => {
        if current.points.is_empty() {
          current.points.push(at);
        }
        let (c1, c2, p) =
          (transform.apply(c1), transform.apply(c2), transform.apply(p));
        cubic(&mut current.points, at, c1, c2, p, tolerance);
        at = p;
      },
      Seg::Close => {
        if !current.points.is_empty() {
          current.closed = true;
          finish(&mut current, &mut out);
        }
        at = start;
      },
    }
  }
  finish(&mut current, &mut out);
  out
}

fn cubic(
  out: &mut Vec<P>,
  p0: P,
  c1: P,
  c2: P,
  p1: P,
  tolerance: f32,
) {
  // Split into n lines, a cubic strays by at most 3/4 × the larger second
  // difference / n².
  let d1 = p0.sub(c1.scale(2.0)).add(c2);
  let d2 = c1.sub(c2.scale(2.0)).add(p1);
  let dev = d1.length().max(d2.length());
  let n = (0.75 * dev / tolerance)
    .sqrt()
    .ceil()
    .clamp(1.0, 1000.0) as usize;
  for i in 1..=n {
    let t = i as f32 / n as f32;
    let u = 1.0 - t;
    let p = p0
      .scale(u * u * u)
      .add(c1.scale(3.0 * u * u * t))
      .add(c2.scale(3.0 * u * t * t))
      .add(p1.scale(t * t * t));
    out.push(p);
  }
}

/// How the ends of an open stroked line are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cap {
  /// Square, at the end.
  #[default]
  Butt,
  /// Round, half a width past the end.
  Round,
  /// Square, half a width past the end.
  Square,
}

/// How corners of a stroked line are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Join {
  /// Pointed, unless longer than the miter limit.
  #[default]
  Miter,
  /// Round.
  Round,
  /// Cut off.
  Bevel,
}

/// A stroke's width and how its ends and corners look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
  pub width: f32,
  pub cap: Cap,
  pub join: Join,
  pub miter_limit: f32,
}

/// Turns stroked lines into shapes that, filled together with the nonzero
/// rule, cover the stroke: a quadrilateral for each line, and joins and
/// caps as their own shapes, all wound the same way.
pub fn stroke(
  lines: &[Polyline],
  stroke: &Stroke,
  tolerance: f32,
) -> Vec<Vec<P>> {
  let hw = stroke.width / 2.0;
  let mut out = Vec::new();
  if hw <= 0.0 {
    return out;
  }
  for line in lines {
    // Drop points that do not move.
    let mut pts: Vec<P> = Vec::with_capacity(line.points.len());
    for &p in &line.points {
      if pts
        .last()
        .is_none_or(|&q: &P| q.sub(p).length() > 1e-4)
      {
        pts.push(p);
      }
    }
    if line.closed
      && pts.len() > 1
      && pts[0].sub(*pts.last().unwrap()).length() <= 1e-4
    {
      pts.pop();
    }
    if pts.len() == 1 {
      // A dot: only round and square ends draw one.
      let p = pts[0];
      match stroke.cap {
        Cap::Round => out.push(circle(p, hw, tolerance)),
        Cap::Square => out.push(vec![
          P::new(p.x - hw, p.y - hw),
          P::new(p.x + hw, p.y - hw),
          P::new(p.x + hw, p.y + hw),
          P::new(p.x - hw, p.y + hw),
        ]),
        Cap::Butt => {},
      }
      continue;
    }
    let n = pts.len();
    let segments = if line.closed { n } else { n - 1 };
    for i in 0..segments {
      let (a, b) = (pts[i], pts[(i + 1) % n]);
      let d = b.sub(a);
      let normal = P::new(-d.y, d.x).scale(hw / d.length());
      out.push(vec![
        a.add(normal),
        b.add(normal),
        b.sub(normal),
        a.sub(normal),
      ]);
    }
    // Joins at each corner.
    let corners: Vec<usize> = if line.closed {
      (0..n).collect()
    } else {
      (1..n - 1).collect()
    };
    for i in corners {
      let prev = pts[(i + n - 1) % n];
      let p = pts[i];
      let next = pts[(i + 1) % n];
      join(&mut out, prev, p, next, hw, stroke, tolerance);
    }
    if !line.closed {
      for (end, inner) in [
        (pts[0], pts[1]),
        (pts[n - 1], pts[n - 2]),
      ] {
        let out_dir = end
          .sub(inner)
          .scale(1.0 / end.sub(inner).length());
        match stroke.cap {
          Cap::Butt => {},
          Cap::Round => out.push(circle(end, hw, tolerance)),
          Cap::Square => {
            let normal = P::new(-out_dir.y, out_dir.x).scale(hw);
            let far = end.add(out_dir.scale(hw));
            out.push(vec![
              end.add(normal),
              far.add(normal),
              far.sub(normal),
              end.sub(normal),
            ]);
          },
        }
      }
    }
  }
  for shape in &mut out {
    if area(shape) < 0.0 {
      shape.reverse();
    }
  }
  out
}

fn join(
  out: &mut Vec<Vec<P>>,
  prev: P,
  p: P,
  next: P,
  hw: f32,
  stroke: &Stroke,
  tolerance: f32,
) {
  let d0 = p.sub(prev).scale(1.0 / p.sub(prev).length());
  let d1 = next.sub(p).scale(1.0 / next.sub(p).length());
  let turn = d0.cross(d1);
  if turn.abs() < 1e-6 && d0.dot(d1) > 0.0 {
    // Straight on: the two lines already meet.
    return;
  }
  match stroke.join {
    Join::Round => out.push(circle(p, hw, tolerance)),
    Join::Bevel | Join::Miter => {
      // The outer side is away from the turn.
      let side = if turn > 0.0 { -1.0 } else { 1.0 };
      let n0 = P::new(-d0.y, d0.x).scale(hw * side);
      let n1 = P::new(-d1.y, d1.x).scale(hw * side);
      let (a, b) = (p.add(n0), p.add(n1));
      // The miter's length over the stroke width is 1 / sin(θ / 2).
      let cos = d0.dot(d1).clamp(-1.0, 1.0);
      let half_sin = ((1.0 - cos) / 2.0).sqrt();
      let ratio = 1.0 / ((1.0 - half_sin * half_sin).sqrt()).max(1e-6);
      if stroke.join == Join::Miter && ratio <= stroke.miter_limit {
        let mid = n0.add(n1);
        let tip = p.add(mid.scale(hw * ratio / mid.length().max(1e-6)));
        out.push(vec![p, a, tip, b]);
      } else {
        out.push(vec![p, a, b]);
      }
    },
  }
}

/// A circle as a polygon close enough at `tolerance`.
pub fn circle(
  c: P,
  r: f32,
  tolerance: f32,
) -> Vec<P> {
  // A chord of angle θ strays r (1 − cos(θ/2)) from the circle.
  let theta = 2.0 * (1.0 - (tolerance / r).min(1.0)).acos();
  let n =
    ((std::f32::consts::TAU / theta.max(1e-3)).ceil() as usize).clamp(8, 512);
  (0..n)
    .map(|i| {
      let a = i as f32 / n as f32 * std::f32::consts::TAU;
      P::new(c.x + r * a.cos(), c.y + r * a.sin())
    })
    .collect()
}

/// The signed area of a polygon, positive when it turns
/// clockwise on screen (y down).
pub fn area(points: &[P]) -> f32 {
  let n = points.len();
  (0..n)
    .map(|i| points[i].cross(points[(i + 1) % n]))
    .sum::<f32>()
    / 2.0
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn path_data_with_implicit_commands_and_tight_numbers() {
    let p = parse("M1,2 3 4l1-1.5.5.5zm0 0h2v3H0V0").unwrap();
    assert_eq!(
      p.0[..4],
      [
        Seg::Move(P::new(1.0, 2.0)),
        Seg::Line(P::new(3.0, 4.0)),
        Seg::Line(P::new(4.0, 2.5)),
        Seg::Line(P::new(4.5, 3.0)),
      ]
    );
    assert_eq!(p.0[4], Seg::Close);
    // After the close, the pen is back at (1, 2).
    assert_eq!(p.0[5], Seg::Move(P::new(1.0, 2.0)));
    assert_eq!(p.0[6], Seg::Line(P::new(3.0, 2.0)));
    assert_eq!(p.0[7], Seg::Line(P::new(3.0, 5.0)));
    assert!(parse("L1 1").is_err(), "must start with a move");
    assert!(parse("1 1").is_err(), "must start with a command");
    assert!(parse("M1").is_err());
  }

  #[test]
  fn arcs_with_packed_flags_reach_their_end() {
    let p = parse("M0 0a5 5 0 0010 0").unwrap();
    let Some(Seg::Cubic(_, _, end)) = p.0.last() else {
      panic!("{p:?}")
    };
    assert_eq!(*end, P::new(10.0, 0.0));
    // A half circle of radius 5 reaches 5 below or above the line.
    let lines = flatten(&p, &Affine::IDENTITY, 0.01);
    let lowest = lines[0]
      .points
      .iter()
      .map(|p| p.y.abs())
      .fold(0.0, f32::max);
    assert!((lowest - 5.0).abs() < 0.05, "{lowest}");
  }

  #[test]
  fn smooth_curves_reflect_the_last_control() {
    let p = parse("M0 0C1 1 2 1 3 0S5 -1 6 0").unwrap();
    assert_eq!(
      p.0[2],
      Seg::Cubic(P::new(4.0, -1.0), P::new(5.0, -1.0), P::new(6.0, 0.0))
    );
  }

  #[test]
  fn transforms_compose_in_order() {
    let t = Affine::translate(10.0, 0.0).then(Affine::scale(2.0, 2.0));
    assert_eq!(t.apply(P::new(1.0, 1.0)), P::new(22.0, 2.0));
    let r = Affine::rotate(90.0).apply(P::new(1.0, 0.0));
    assert!((r.x).abs() < 1e-6 && (r.y - 1.0).abs() < 1e-6);
  }

  #[test]
  fn strokes_are_wound_one_way() {
    let line = Polyline {
      points: vec![
        P::new(0.0, 0.0),
        P::new(10.0, 0.0),
        P::new(10.0, 10.0),
      ],
      closed: false,
    };
    let style = Stroke {
      width: 2.0,
      cap: Cap::Round,
      join: Join::Miter,
      miter_limit: 4.0,
    };
    let shapes = stroke(&[line], &style, 0.05);
    // Two lines, one miter and two round ends.
    assert_eq!(shapes.len(), 5);
    assert!(shapes.iter().all(|s| area(s) > 0.0));
    // The miter reaches the outer corner.
    let miter = &shapes[2];
    assert!(miter.contains(&P::new(11.0, -1.0)), "{miter:?}");
  }
}
