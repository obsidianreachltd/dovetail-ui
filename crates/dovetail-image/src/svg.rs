//! The SVG subset icons are read from (D0023).
//!
//! - Elements: `svg` with `viewBox`, `g`, `path`, `rect`, `circle`,
//!   `ellipse`, `line`, `polyline` and `polygon`.
//! - Painting: `fill`, `fill-rule`, `fill-opacity`, `stroke`,
//!   `stroke-width`, `stroke-linecap`, `stroke-linejoin`,
//!   `stroke-miterlimit`, `stroke-opacity`, `opacity`, `display` and
//!   `visibility`, as attributes or in a `style` attribute.
//! - `transform`: `matrix`, `translate`, `scale`, `rotate`, `skewX` and
//!   `skewY`.
//!
//! Colours (D0024): `currentColor`, or no paint at all, is the colour of
//! the text around the icon. An icon whose painted parts all have one
//! colour is drawn in the text's colour too; one with two or more keeps
//! them. Gradients, patterns and other paint servers count as the text's
//! colour. Gradients,
//! text, filters, masks, `use` and style sheets are not read; elements this
//! subset does not know are skipped.

use crate::path::{self, Affine, Cap, Join, P, Path, Seg, Stroke};
use crate::raster::FillRule;
use crate::xml::{self, Node};

/// Why an icon's SVG could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SvgError {
  /// The XML is broken at this byte.
  Xml(usize, &'static str),
  /// The root element is not `svg`.
  NotSvg,
  /// There is no `viewBox`, and no width and height either.
  NoSize,
  /// Path data is broken at this byte of the `d` attribute.
  Path(usize),
}

impl std::fmt::Display for SvgError {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    match self {
      SvgError::Xml(at, what) => write!(f, "broken XML at byte {at}: {what}"),
      SvgError::NotSvg => f.write_str("the root element is not svg"),
      SvgError::NoSize => {
        f.write_str("the SVG has no viewBox, width or height")
      },
      SvgError::Path(at) => write!(f, "broken path data at byte {at}"),
    }
  }
}

impl std::error::Error for SvgError {}

/// One shape to draw: a path with its transform, and how it is painted.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
  pub path: Path,
  pub transform: Affine,
  /// The rule to fill with, or `None` when it is not filled.
  pub fill: Option<FillRule>,
  pub fill_opacity: f32,
  /// What the fill is painted with.
  pub fill_ink: Ink,
  pub stroke: Option<Stroke>,
  pub stroke_opacity: f32,
  /// What the stroke is painted with.
  pub stroke_ink: Ink,
}

/// What a part of an icon is painted with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ink {
  /// The colour of the text around the icon.
  Current,
  /// A colour of its own: straight sRGB red, green, blue and alpha, from 0
  /// to 1.
  Color([f32; 4]),
}

/// An icon's shapes and the box they are drawn in.
#[derive(Clone, Debug, PartialEq)]
pub struct Svg {
  /// x, y, width and height of the `viewBox`.
  pub view_box: [f32; 4],
  pub shapes: Vec<Shape>,
}

/// Painting that groups pass to what is inside them.
#[derive(Clone, Copy)]
struct Paint {
  fill: Option<Ink>,
  fill_rule: FillRule,
  fill_opacity: f32,
  stroke: Option<Ink>,
  stroke_width: f32,
  cap: Cap,
  join: Join,
  miter_limit: f32,
  stroke_opacity: f32,
  /// `opacity` of every group around, multiplied.
  opacity: f32,
  visible: bool,
}

impl Default for Paint {
  fn default() -> Self {
    Paint {
      fill: Some(Ink::Current),
      fill_rule: FillRule::NonZero,
      fill_opacity: 1.0,
      stroke: None,
      stroke_width: 1.0,
      cap: Cap::Butt,
      join: Join::Miter,
      miter_limit: 4.0,
      stroke_opacity: 1.0,
      opacity: 1.0,
      visible: true,
    }
  }
}

/// Reads an icon.
pub fn parse(text: &str) -> Result<Svg, SvgError> {
  let root = xml::parse(text).map_err(|e| SvgError::Xml(e.at, e.what))?;
  if root.name != "svg" {
    return Err(SvgError::NotSvg);
  }
  let numbers = |s: &str| -> Vec<f32> { list(s) };
  let view_box = match root.get("viewBox").map(numbers) {
    Some(v) if v.len() == 4 && v[2] > 0.0 && v[3] > 0.0 => {
      [v[0], v[1], v[2], v[3]]
    },
    _ => {
      let w = root.get("width").and_then(length);
      let h = root.get("height").and_then(length);
      match (w, h) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => [0.0, 0.0, w, h],
        _ => return Err(SvgError::NoSize),
      }
    },
  };
  let mut shapes = Vec::new();
  let paint = apply(Paint::default(), &root);
  walk(&root, paint, Affine::IDENTITY, &mut shapes)?;
  Ok(Svg { view_box, shapes })
}

fn walk(
  node: &Node,
  paint: Paint,
  transform: Affine,
  out: &mut Vec<Shape>,
) -> Result<(), SvgError> {
  for child in &node.children {
    let paint = apply(paint, child);
    if !paint.visible {
      continue;
    }
    let transform = match child.get("transform") {
      Some(t) => parse_transform(t).then(transform),
      None => transform,
    };
    let num = |name: &str| child.get(name).and_then(length).unwrap_or(0.0);
    let path = match child.name.as_str() {
      "g" | "a" | "switch" => {
        walk(child, paint, transform, out)?;
        continue;
      },
      "path" => match child.get("d") {
        Some(d) => path::parse(d).map_err(|e| SvgError::Path(e.0))?,
        None => continue,
      },
      "rect" => {
        let (x, y, w, h) = (num("x"), num("y"), num("width"), num("height"));
        if w <= 0.0 || h <= 0.0 {
          continue;
        }
        let rx = child.get("rx").and_then(length);
        let ry = child.get("ry").and_then(length);
        let (rx, ry) = match (rx, ry) {
          (Some(rx), Some(ry)) => (rx, ry),
          (Some(r), None) | (None, Some(r)) => (r, r),
          (None, None) => (0.0, 0.0),
        };
        rect(x, y, w, h, rx.min(w / 2.0), ry.min(h / 2.0))
      },
      "circle" => {
        let r = num("r");
        if r <= 0.0 {
          continue;
        }
        ellipse(num("cx"), num("cy"), r, r)
      },
      "ellipse" => {
        let (rx, ry) = (num("rx"), num("ry"));
        if rx <= 0.0 || ry <= 0.0 {
          continue;
        }
        ellipse(num("cx"), num("cy"), rx, ry)
      },
      "line" => Path(vec![
        Seg::Move(P::new(num("x1"), num("y1"))),
        Seg::Line(P::new(num("x2"), num("y2"))),
      ]),
      "polyline" | "polygon" => {
        let v = list(child.get("points").unwrap_or(""));
        let mut segs: Vec<Seg> = v
          .chunks_exact(2)
          .enumerate()
          .map(|(i, p)| {
            let p = P::new(p[0], p[1]);
            if i == 0 { Seg::Move(p) } else { Seg::Line(p) }
          })
          .collect();
        if segs.is_empty() {
          continue;
        }
        if child.name == "polygon" {
          segs.push(Seg::Close);
        }
        Path(segs)
      },
      // Everything else, such as defs, title and text, is not drawn.
      _ => continue,
    };
    // A line has no inside.
    let fill_ink = paint.fill.filter(|_| child.name != "line");
    let fill = fill_ink.map(|_| paint.fill_rule);
    let stroke = paint.stroke.map(|_| Stroke {
      width: paint.stroke_width,
      cap: paint.cap,
      join: paint.join,
      miter_limit: paint.miter_limit.max(1.0),
    });
    if fill.is_none() && stroke.is_none() {
      continue;
    }
    out.push(Shape {
      path,
      transform,
      fill,
      fill_opacity: paint.fill_opacity * paint.opacity,
      fill_ink: fill_ink.unwrap_or(Ink::Current),
      stroke_ink: paint.stroke.unwrap_or(Ink::Current),
      stroke,
      stroke_opacity: paint.stroke_opacity * paint.opacity,
    });
  }
  Ok(())
}

/// The painting for `node`: its parent's, changed by its attributes and
/// its `style`, which wins.
fn apply(
  mut paint: Paint,
  node: &Node,
) -> Paint {
  // `opacity` belongs to the element, so it multiplies.
  let mut declarations: Vec<(&str, &str)> = node
    .attributes
    .iter()
    .map(|(n, v)| (n.as_str(), v.as_str()))
    .collect();
  if let Some(style) = node.get("style") {
    for d in style.split(';') {
      if let Some((n, v)) = d.split_once(':') {
        declarations.push((n.trim(), v.trim()));
      }
    }
  }
  for (name, value) in declarations {
    let value = value.trim();
    let number = || {
      value
        .parse::<f32>()
        .ok()
        .map(|v| v.clamp(0.0, 1.0))
    };
    match name {
      "fill" => paint.fill = ink(value),
      "stroke" => paint.stroke = ink(value),
      "fill-rule" => {
        paint.fill_rule = if value == "evenodd" {
          FillRule::EvenOdd
        } else {
          FillRule::NonZero
        }
      },
      "fill-opacity" => paint.fill_opacity = number().unwrap_or(1.0),
      "stroke-opacity" => paint.stroke_opacity = number().unwrap_or(1.0),
      "opacity" => paint.opacity *= number().unwrap_or(1.0),
      "stroke-width" => {
        if let Some(w) = length(value) {
          paint.stroke_width = w.max(0.0);
        }
      },
      "stroke-linecap" => {
        paint.cap = match value {
          "round" => Cap::Round,
          "square" => Cap::Square,
          _ => Cap::Butt,
        }
      },
      "stroke-linejoin" => {
        paint.join = match value {
          "round" => Join::Round,
          "bevel" => Join::Bevel,
          _ => Join::Miter,
        }
      },
      "stroke-miterlimit" => {
        if let Ok(v) = value.parse() {
          paint.miter_limit = v;
        }
      },
      "display" if value == "none" => paint.visible = false,
      "visibility" => paint.visible = value != "hidden" && value != "collapse",
      _ => {},
    }
  }
  paint
}

/// A paint: `None` for `none`, a colour, or the text's colour for
/// `currentColor` and anything not read.
fn ink(value: &str) -> Option<Ink> {
  if value == "none" {
    return None;
  }
  Some(color(value).map_or(Ink::Current, Ink::Color))
}

/// A CSS colour: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`,
/// `rgba()`, or one of the basic names, as straight sRGB from 0 to 1.
pub(crate) fn color(value: &str) -> Option<[f32; 4]> {
  let v = value.trim().to_ascii_lowercase();
  if let Some(hex) = v.strip_prefix('#') {
    let digit = |i: usize| u8::from_str_radix(hex.get(i..i + 1)?, 16).ok();
    let pair = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    let c = |b: u8| f32::from(b) / 255.0;
    return match hex.len() {
      3 | 4 => {
        let n = |i| digit(i).map(|d| c(d * 17));
        let a = if hex.len() == 4 { n(3)? } else { 1.0 };
        Some([n(0)?, n(1)?, n(2)?, a])
      },
      6 | 8 => {
        let n = |i| pair(i).map(c);
        let a = if hex.len() == 8 { n(6)? } else { 1.0 };
        Some([n(0)?, n(2)?, n(4)?, a])
      },
      _ => None,
    };
  }
  if let Some(args) = v
    .strip_prefix("rgba(")
    .or_else(|| v.strip_prefix("rgb("))
    .and_then(|a| a.strip_suffix(')'))
  {
    let parts: Vec<&str> = args
      .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
      .filter(|p| !p.is_empty())
      .collect();
    if parts.len() < 3 {
      return None;
    }
    let channel = |p: &str| -> Option<f32> {
      match p.strip_suffix('%') {
        Some(n) => n.parse::<f32>().ok().map(|n| n / 100.0),
        None => p.parse::<f32>().ok().map(|n| n / 255.0),
      }
    };
    let alpha = match parts.get(3) {
      Some(p) => match p.strip_suffix('%') {
        Some(n) => n.parse::<f32>().ok()? / 100.0,
        None => p.parse::<f32>().ok()?,
      },
      None => 1.0,
    };
    return Some([
      channel(parts[0])?.clamp(0.0, 1.0),
      channel(parts[1])?.clamp(0.0, 1.0),
      channel(parts[2])?.clamp(0.0, 1.0),
      alpha.clamp(0.0, 1.0),
    ]);
  }
  let rgb: u32 = match v.as_str() {
    "black" => 0x000000,
    "white" => 0xffffff,
    "red" => 0xff0000,
    "lime" => 0x00ff00,
    "green" => 0x008000,
    "blue" => 0x0000ff,
    "yellow" => 0xffff00,
    "orange" => 0xffa500,
    "purple" => 0x800080,
    "gray" | "grey" => 0x808080,
    "silver" => 0xc0c0c0,
    "maroon" => 0x800000,
    "olive" => 0x808000,
    "teal" => 0x008080,
    "navy" => 0x000080,
    "fuchsia" | "magenta" => 0xff00ff,
    "aqua" | "cyan" => 0x00ffff,
    "transparent" => return Some([0.0; 4]),
    _ => return None,
  };
  let c = |shift: u32| ((rgb >> shift) & 0xff) as f32 / 255.0;
  Some([c(16), c(8), c(0), 1.0])
}

/// A length, with any `px` dropped. Other units and percentages are not
/// read.
fn length(s: &str) -> Option<f32> {
  let s = s.trim();
  let s = s.strip_suffix("px").unwrap_or(s);
  s.parse().ok()
}

/// Numbers separated by spaces or commas.
fn list(s: &str) -> Vec<f32> {
  s.split(|c: char| c.is_whitespace() || c == ',')
    .filter(|t| !t.is_empty())
    .filter_map(|t| t.parse().ok())
    .collect()
}

fn parse_transform(s: &str) -> Affine {
  let mut t = Affine::IDENTITY;
  let mut rest = s;
  // Each function applies to what comes after it, so the first is
  // outermost.
  let mut parts = Vec::new();
  while let Some(open) = rest.find('(') {
    let name = rest[..open].trim().trim_start_matches(',').trim();
    let Some(close) = rest[open..].find(')') else {
      break;
    };
    let args = list(&rest[open + 1..open + close]);
    let a = |i: usize| args.get(i).copied().unwrap_or(0.0);
    let m = match name {
      "matrix" if args.len() == 6 => {
        Affine([a(0), a(1), a(2), a(3), a(4), a(5)])
      },
      "translate" => Affine::translate(a(0), a(1)),
      "scale" => Affine::scale(a(0), if args.len() > 1 { a(1) } else { a(0) }),
      "rotate" if args.len() == 3 => Affine::translate(-a(1), -a(2))
        .then(Affine::rotate(a(0)))
        .then(Affine::translate(a(1), a(2))),
      "rotate" => Affine::rotate(a(0)),
      "skewX" => Affine([
        1.0,
        0.0,
        a(0).to_radians().tan(),
        1.0,
        0.0,
        0.0,
      ]),
      "skewY" => Affine([
        1.0,
        a(0).to_radians().tan(),
        0.0,
        1.0,
        0.0,
        0.0,
      ]),
      _ => Affine::IDENTITY,
    };
    parts.push(m);
    rest = &rest[open + close + 1..];
  }
  for m in parts.into_iter().rev() {
    t = t.then(m);
  }
  t
}

fn ellipse(
  cx: f32,
  cy: f32,
  rx: f32,
  ry: f32,
) -> Path {
  // Four cubic quarters: the usual 0.5523 of the radius for the controls.
  let k = 0.552_284_8;
  let (ox, oy) = (rx * k, ry * k);
  Path(vec![
    Seg::Move(P::new(cx + rx, cy)),
    Seg::Cubic(
      P::new(cx + rx, cy + oy),
      P::new(cx + ox, cy + ry),
      P::new(cx, cy + ry),
    ),
    Seg::Cubic(
      P::new(cx - ox, cy + ry),
      P::new(cx - rx, cy + oy),
      P::new(cx - rx, cy),
    ),
    Seg::Cubic(
      P::new(cx - rx, cy - oy),
      P::new(cx - ox, cy - ry),
      P::new(cx, cy - ry),
    ),
    Seg::Cubic(
      P::new(cx + ox, cy - ry),
      P::new(cx + rx, cy - oy),
      P::new(cx + rx, cy),
    ),
    Seg::Close,
  ])
}

fn rect(
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  rx: f32,
  ry: f32,
) -> Path {
  if rx <= 0.0 || ry <= 0.0 {
    return Path(vec![
      Seg::Move(P::new(x, y)),
      Seg::Line(P::new(x + w, y)),
      Seg::Line(P::new(x + w, y + h)),
      Seg::Line(P::new(x, y + h)),
      Seg::Close,
    ]);
  }
  let k = 0.552_284_8;
  let (ox, oy) = (rx * k, ry * k);
  let (r, b) = (x + w, y + h);
  Path(vec![
    Seg::Move(P::new(x + rx, y)),
    Seg::Line(P::new(r - rx, y)),
    Seg::Cubic(
      P::new(r - rx + ox, y),
      P::new(r, y + ry - oy),
      P::new(r, y + ry),
    ),
    Seg::Line(P::new(r, b - ry)),
    Seg::Cubic(
      P::new(r, b - ry + oy),
      P::new(r - rx + ox, b),
      P::new(r - rx, b),
    ),
    Seg::Line(P::new(x + rx, b)),
    Seg::Cubic(
      P::new(x + rx - ox, b),
      P::new(x, b - ry + oy),
      P::new(x, b - ry),
    ),
    Seg::Line(P::new(x, y + ry)),
    Seg::Cubic(
      P::new(x, y + ry - oy),
      P::new(x + rx - ox, y),
      P::new(x + rx, y),
    ),
    Seg::Close,
  ])
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn groups_pass_painting_down_and_style_wins() {
    let svg = parse(
      r##"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <g stroke-linecap="round" opacity="0.5">
          <path d="M4 4h16" style="stroke-width: 3; fill: red"/>
        </g>
        <circle cx="12" cy="12" r="3" fill="#000" stroke="none"/>
        <rect width="0" height="5"/>
        <text>skipped</text>
      </svg>"##,
    )
    .unwrap();
    assert_eq!(svg.view_box, [0.0, 0.0, 24.0, 24.0]);
    assert_eq!(svg.shapes.len(), 2);
    let line = &svg.shapes[0];
    let s = line.stroke.unwrap();
    assert_eq!((s.width, s.cap), (3.0, Cap::Round));
    assert_eq!(line.fill, Some(FillRule::NonZero), "style's fill wins");
    assert_eq!(line.stroke_opacity, 0.5);
    let dot = &svg.shapes[1];
    assert!(dot.fill.is_some() && dot.stroke.is_none());
  }

  #[test]
  fn colours_and_the_text_colour() {
    assert_eq!(color("#f00"), Some([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(
      color("#00ff0080").map(|c| (c[1], (c[3] * 255.0).round())),
      Some((1.0, 128.0))
    );
    assert_eq!(color("rgb(0, 0, 255)"), Some([0.0, 0.0, 1.0, 1.0]));
    assert_eq!(color("rgba(100%, 0%, 0%, 0.5)"), Some([1.0, 0.0, 0.0, 0.5]));
    assert_eq!(color("Navy").map(|c| c[2]), Some(128.0 / 255.0));
    assert_eq!(color("url(#g)"), None);
    assert_eq!(ink("currentColor"), Some(Ink::Current));
    assert_eq!(ink("none"), None);
    let svg = parse(r##"<svg viewBox="0 0 2 2"><rect width="1" height="1" fill="#f00"/><rect x="1" width="1" height="1"/></svg>"##).unwrap();
    assert_eq!(svg.shapes[0].fill_ink, Ink::Color([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(
      svg.shapes[1].fill_ink,
      Ink::Current,
      "no paint is the text's colour"
    );
  }

  #[test]
  fn transforms_apply_right_to_left() {
    let t = parse_transform("translate(10 0) scale(2)");
    assert_eq!(t.apply(P::new(1.0, 1.0)), P::new(12.0, 2.0));
    let r = parse_transform("rotate(90 5 5)").apply(P::new(10.0, 5.0));
    assert!(
      (r.x - 5.0).abs() < 1e-4 && (r.y - 10.0).abs() < 1e-4,
      "{r:?}"
    );
  }

  #[test]
  fn sizes_without_a_view_box() {
    let svg = parse(r#"<svg width="16px" height="8"><path d="M0 0h1"/></svg>"#)
      .unwrap();
    assert_eq!(svg.view_box, [0.0, 0.0, 16.0, 8.0]);
    assert_eq!(parse("<svg/>"), Err(SvgError::NoSize));
    assert_eq!(parse("<html/>"), Err(SvgError::NotSvg));
    assert!(matches!(
      parse(r#"<svg viewBox="0 0 1 1"><path d="M0 0 L"/></svg>"#),
      Err(SvgError::Path(_))
    ));
  }
}
