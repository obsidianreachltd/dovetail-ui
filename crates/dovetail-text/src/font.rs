//! Reading one TrueType or OpenType font: tables, metrics, character map,
//! advances, kerning and glyph outlines (`glyf`, or `CFF` through
//! `cff.rs`).

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::cff::Cff;
use crate::kern::Kerning;
use crate::language::Language;
use crate::outline::{Outline, Point};
use crate::read::Data;

/// Why a font could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontError {
  /// The data is not a TrueType or OpenType font or collection.
  NotAFont,
  /// A collection has fewer fonts than the index asked for.
  NoSuchFont(u32),
  /// A table the font needs is missing or too short.
  MissingTable(&'static str),
  /// The outlines are in a format not read: `CFF2`, used by variable
  /// OpenType fonts.
  UnsupportedOutlines,
}

impl fmt::Display for FontError {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    match self {
      FontError::NotAFont => f.write_str("not a TrueType or OpenType font"),
      FontError::NoSuchFont(i) => {
        write!(f, "the collection has no font number {i}")
      },
      FontError::MissingTable(t) => {
        write!(f, "the font has no usable `{t}` table")
      },
      FontError::UnsupportedOutlines => {
        f.write_str("the font uses CFF2 outlines, which are not read")
      },
    }
  }
}

impl std::error::Error for FontError {}

/// Vertical metrics of a font, in pixels at one size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineMetrics {
  /// Distance from the baseline up to the top of the tallest letters.
  pub ascent: f32,
  /// Distance from the baseline down to the bottom of descenders, as a
  /// positive number.
  pub descent: f32,
  /// Extra space between lines.
  pub line_gap: f32,
}

impl LineMetrics {
  /// The height of one line: ascent, descent and line gap.
  pub fn line_height(&self) -> f32 {
    self.ascent + self.descent + self.line_gap
  }
}

/// Which character map subtable a font uses.
#[derive(Clone, Copy, Debug)]
enum Cmap {
  /// Format 4: segments, for the Basic Multilingual Plane.
  Segments(usize),
  /// Format 12: groups, for all of Unicode.
  Groups(usize),
}

/// One TrueType font, read from file data that it shares.
#[derive(Clone)]
pub struct Font {
  data: Arc<[u8]>,
  tables: HashMap<[u8; 4], (usize, usize)>,
  units_per_em: f32,
  long_loca: bool,
  num_glyphs: u16,
  num_h_metrics: u16,
  ascender: i16,
  descender: i16,
  line_gap: i16,
  cmap: Cmap,
  family: String,
  kerning: Kerning,
  /// CFF outlines, for fonts without `glyf`.
  cff: Option<Cff>,
  /// The CJK languages the font is made for, as `OS/2` code page bits.
  languages: u32,
}

impl fmt::Debug for Font {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    f.debug_struct("Font")
      .field("family", &self.family)
      .field("glyphs", &self.num_glyphs)
      .finish()
  }
}

impl Font {
  /// Reads font number `index` from `data`: 0 for a single font, or the
  /// position in a collection (`.ttc`).
  pub fn from_data(
    data: impl Into<Arc<[u8]>>,
    index: u32,
  ) -> Result<Font, FontError> {
    let data: Arc<[u8]> = data.into();
    let d = Data(&data);
    let start = match d.tag(0).ok_or(FontError::NotAFont)? {
      tag if &tag == b"ttcf" => {
        let count = d.u32(8).ok_or(FontError::NotAFont)?;
        if index >= count {
          return Err(FontError::NoSuchFont(index));
        }
        d.u32(12 + 4 * index as usize)
          .ok_or(FontError::NotAFont)? as usize
      },
      _ if index == 0 => 0,
      _ => return Err(FontError::NoSuchFont(index)),
    };
    match d.u32(start).ok_or(FontError::NotAFont)? {
      0x0001_0000 | 0x7472_7565 => {}, // TrueType, or 'true' (Apple)
      0x4f54_544f => {},               // 'OTTO': OpenType with CFF outlines
      _ => return Err(FontError::NotAFont),
    }
    let count = d.u16(start + 4).ok_or(FontError::NotAFont)? as usize;
    let mut tables = HashMap::with_capacity(count);
    for i in 0..count {
      let record = start + 12 + 16 * i;
      let tag = d.tag(record).ok_or(FontError::NotAFont)?;
      let offset = d.u32(record + 8).ok_or(FontError::NotAFont)? as usize;
      let length = d.u32(record + 12).ok_or(FontError::NotAFont)? as usize;
      if d.slice(offset, length).is_some() {
        tables.insert(tag, (offset, length));
      }
    }
    let table =
      |tag: &[u8; 4], name, min: usize| -> Result<Data<'_>, FontError> {
        let &(offset, length) = tables
          .get(tag)
          .ok_or(FontError::MissingTable(name))?;
        let t = d
          .slice(offset, length)
          .ok_or(FontError::MissingTable(name))?;
        if length < min {
          Err(FontError::MissingTable(name))
        } else {
          Ok(t)
        }
      };
    // TrueType outlines, or else CFF ones.
    let cff = if tables.contains_key(b"glyf") {
      table(b"loca", "loca", 2)?;
      None
    } else if tables.contains_key(b"CFF ") {
      Some(
        Cff::read(table(b"CFF ", "CFF ", 4)?)
          .ok_or(FontError::MissingTable("CFF "))?,
      )
    } else if tables.contains_key(b"CFF2") {
      return Err(FontError::UnsupportedOutlines);
    } else {
      return Err(FontError::MissingTable("glyf"));
    };
    let head = table(b"head", "head", 54)?;
    let hhea = table(b"hhea", "hhea", 36)?;
    let maxp = table(b"maxp", "maxp", 6)?;
    table(b"hmtx", "hmtx", 4)?;
    let cmap_table = table(b"cmap", "cmap", 4)?;
    let units_per_em = f32::from(
      head
        .u16(18)
        .filter(|&u| u > 0)
        .ok_or(FontError::MissingTable("head"))?,
    );
    let cmap =
      choose_cmap(cmap_table).ok_or(FontError::MissingTable("cmap"))?;
    let family = tables
      .get(b"name")
      .and_then(|&(o, l)| d.slice(o, l))
      .and_then(family_name)
      .unwrap_or_default();
    let optional = |tag: &[u8; 4]| {
      tables
        .get(tag)
        .and_then(|&(o, l)| d.slice(o, l))
        .unwrap_or(Data(&[]))
    };
    let kerning = Kerning::find(optional(b"GPOS"), optional(b"kern"));
    // A region at the end of the family name ("Noto Sans CJK JP") decides;
    // otherwise the code pages in `OS/2` (version 1 or later) do.
    let languages = match Language::of_family(&family) {
      Some(language) => language.code_pages(),
      None => {
        let os2 = optional(b"OS/2");
        match os2.u16(0) {
          Some(version) if version >= 1 => os2.u32(78).unwrap_or(0),
          _ => 0,
        }
      },
    };
    let font = Font {
      languages,
      cff,
      kerning,
      units_per_em,
      long_loca: head.i16(50) == Some(1),
      num_glyphs: maxp.u16(4).unwrap_or(0),
      num_h_metrics: hhea.u16(34).unwrap_or(1).max(1),
      ascender: hhea.i16(4).unwrap_or(0),
      descender: hhea.i16(6).unwrap_or(0),
      line_gap: hhea.i16(8).unwrap_or(0),
      cmap,
      family,
      tables,
      data,
    };
    Ok(font)
  }

  pub(crate) fn table(
    &self,
    tag: &[u8; 4],
  ) -> Data<'_> {
    match self.tables.get(tag) {
      Some(&(offset, length)) => Data(&self.data[offset..offset + length]),
      None => Data(&[]),
    }
  }

  /// The font's family name, such as "Segoe UI", or empty if it has none.
  pub fn family(&self) -> &str {
    &self.family
  }

  /// Font units in one em (the font size).
  pub fn units_per_em(&self) -> f32 {
    self.units_per_em
  }

  /// How many glyphs the font has.
  pub fn glyph_count(&self) -> u16 {
    self.num_glyphs
  }

  /// The font's line metrics at `size` pixels.
  pub fn line_metrics(
    &self,
    size: f32,
  ) -> LineMetrics {
    let s = size / self.units_per_em;
    LineMetrics {
      ascent: f32::from(self.ascender) * s,
      descent: -f32::from(self.descender) * s,
      line_gap: f32::from(self.line_gap.max(0)) * s,
    }
  }

  /// The glyph for `c`, or `None` when the font does not have it.
  pub fn glyph_index(
    &self,
    c: char,
  ) -> Option<u16> {
    let cmap = self.table(b"cmap");
    let c = c as u32;
    let glyph = match self.cmap {
      Cmap::Segments(at) => lookup_segments(cmap, at, c),
      Cmap::Groups(at) => lookup_groups(cmap, at, c),
    }?;
    (glyph != 0 && glyph < self.num_glyphs).then_some(glyph)
  }

  /// How far the pen moves after `glyph`, in font units.
  pub fn advance(
    &self,
    glyph: u16,
  ) -> f32 {
    let hmtx = self.table(b"hmtx");
    let i = glyph.min(self.num_h_metrics - 1) as usize;
    f32::from(hmtx.u16(4 * i).unwrap_or(0))
  }

  /// How much `left`'s advance changes when `right` follows it, in font
  /// units: the font's kerning. Negative moves the two closer.
  pub fn kerning(
    &self,
    left: u16,
    right: u16,
  ) -> f32 {
    self
      .kerning
      .pair(self.table(b"GPOS"), self.table(b"kern"), left, right) as f32
  }

  /// True when the font is made for `language`'s way of drawing Han
  /// characters. Always false for [`Language::Other`].
  pub fn is_for(
    &self,
    language: Language,
  ) -> bool {
    self.languages & language.code_pages() != 0
  }

  /// True when the font has kerning that Dovetail reads.
  pub fn has_kerning(&self) -> bool {
    !matches!(self.kerning, Kerning::None)
  }

  /// The outline of `glyph`, in font units with y up. Empty for glyphs that
  /// draw nothing, and for glyphs that cannot be read.
  pub fn outline(
    &self,
    glyph: u16,
  ) -> Outline {
    if let Some(cff) = &self.cff {
      return cff.outline(self.table(b"CFF "), glyph);
    }
    let mut outline = Outline::default();
    if let Some((min, max)) =
      self.add_glyph(glyph, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], &mut outline, 0)
    {
      outline.bounds = (min, max);
    }
    outline
  }

  /// The bytes of `glyph` in the `glyf` table.
  fn glyph_data(
    &self,
    glyph: u16,
  ) -> Option<Data<'_>> {
    let loca = self.table(b"loca");
    let i = glyph as usize;
    let (start, end) = if self.long_loca {
      (loca.u32(4 * i)? as usize, loca.u32(4 * i + 4)? as usize)
    } else {
      (
        loca.u16(2 * i)? as usize * 2,
        loca.u16(2 * i + 2)? as usize * 2,
      )
    };
    if end <= start {
      return None;
    }
    self.table(b"glyf").slice(start, end - start)
  }

  /// Adds `glyph`'s contours to `outline`, transformed by the 2 × 3 matrix
  /// `m` (a, b, c, d, dx, dy). Returns the transformed bounding box.
  fn add_glyph(
    &self,
    glyph: u16,
    m: [f32; 6],
    outline: &mut Outline,
    depth: u32,
  ) -> Option<(Point, Point)> {
    if depth > 8 {
      return None; // A composite that refers to itself.
    }
    let g = self.glyph_data(glyph)?;
    let contours = g.i16(0)?;
    let apply = |x: f32, y: f32| {
      Point::new(m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    };
    if contours >= 0 {
      let bounds = [
        g.i16(2)?,
        g.i16(4)?,
        g.i16(6)?,
        g.i16(8)?,
      ]
      .map(f32::from);
      let corners = [
        apply(bounds[0], bounds[1]),
        apply(bounds[2], bounds[3]),
      ];
      add_simple(g, contours as usize, &apply, outline)?;
      let min = Point::new(
        corners[0].x.min(corners[1].x),
        corners[0].y.min(corners[1].y),
      );
      let max = Point::new(
        corners[0].x.max(corners[1].x),
        corners[0].y.max(corners[1].y),
      );
      return Some((min, max));
    }
    // A composite glyph: other glyphs, each moved and possibly scaled.
    let mut at = 10;
    let mut bounds: Option<(Point, Point)> = None;
    loop {
      let flags = g.u16(at)?;
      let component = g.u16(at + 2)?;
      at += 4;
      let (dx, dy) = if flags & 0x0001 != 0 {
        at += 4;
        (f32::from(g.i16(at - 4)?), f32::from(g.i16(at - 2)?))
      } else {
        at += 2;
        (
          f32::from(g.u8(at - 2)? as i8),
          f32::from(g.u8(at - 1)? as i8),
        )
      };
      let (a, b, c, d) = if flags & 0x0008 != 0 {
        at += 2;
        let s = g.f2dot14(at - 2)?;
        (s, 0.0, 0.0, s)
      } else if flags & 0x0040 != 0 {
        at += 4;
        (g.f2dot14(at - 4)?, 0.0, 0.0, g.f2dot14(at - 2)?)
      } else if flags & 0x0080 != 0 {
        at += 8;
        (
          g.f2dot14(at - 8)?,
          g.f2dot14(at - 6)?,
          g.f2dot14(at - 4)?,
          g.f2dot14(at - 2)?,
        )
      } else {
        (1.0, 0.0, 0.0, 1.0)
      };
      // Offsets that match points (rather than x and y values) are rare and
      // are treated as no offset.
      let (dx, dy) = if flags & 0x0002 != 0 {
        (dx, dy)
      } else {
        (0.0, 0.0)
      };
      // Combine: the component's transform first, then the parent's.
      let child = [
        m[0] * a + m[2] * b,
        m[1] * a + m[3] * b,
        m[0] * c + m[2] * d,
        m[1] * c + m[3] * d,
        m[0] * dx + m[2] * dy + m[4],
        m[1] * dx + m[3] * dy + m[5],
      ];
      if let Some((min, max)) =
        self.add_glyph(component, child, outline, depth + 1)
      {
        bounds = Some(match bounds {
          Some((lo, hi)) => (
            Point::new(lo.x.min(min.x), lo.y.min(min.y)),
            Point::new(hi.x.max(max.x), hi.y.max(max.y)),
          ),
          None => (min, max),
        });
      }
      if flags & 0x0020 == 0 {
        return bounds;
      }
    }
  }
}

/// Reads a simple glyph's contours.
fn add_simple(
  g: Data<'_>,
  contours: usize,
  apply: &dyn Fn(f32, f32) -> Point,
  outline: &mut Outline,
) -> Option<()> {
  let mut ends = Vec::with_capacity(contours);
  for i in 0..contours {
    ends.push(g.u16(10 + 2 * i)? as usize);
  }
  let count = ends.last().map_or(0, |&e| e + 1);
  let instructions = g.u16(10 + 2 * contours)? as usize;
  let mut at = 12 + 2 * contours + instructions;
  let mut flags = Vec::with_capacity(count);
  while flags.len() < count {
    let flag = g.u8(at)?;
    at += 1;
    flags.push(flag);
    if flag & 0x08 != 0 {
      let repeat = g.u8(at)?;
      at += 1;
      for _ in 0..repeat {
        flags.push(flag);
      }
    }
  }
  flags.truncate(count);
  let mut read_coords = |short: u8, same: u8| -> Option<Vec<f32>> {
    let mut value = 0i32;
    let mut out = Vec::with_capacity(count);
    for &flag in &flags {
      if flag & short != 0 {
        let delta = i32::from(g.u8(at)?);
        at += 1;
        value += if flag & same != 0 { delta } else { -delta };
      } else if flag & same == 0 {
        value += i32::from(g.i16(at)?);
        at += 2;
      }
      out.push(value as f32);
    }
    Some(out)
  };
  let xs = read_coords(0x02, 0x10)?;
  let ys = read_coords(0x04, 0x20)?;
  let mut start = 0;
  for end in ends {
    if end < start || end >= count {
      return None;
    }
    let points: Vec<(Point, bool)> = (start..=end)
      .map(|i| (apply(xs[i], ys[i]), flags[i] & 0x01 != 0))
      .collect();
    outline.add_contour(&points);
    start = end + 1;
  }
  Some(())
}

/// Chooses the best Unicode character map: format 12, then format 4.
fn choose_cmap(cmap: Data<'_>) -> Option<Cmap> {
  let count = cmap.u16(2)? as usize;
  let mut best: Option<(u8, Cmap)> = None;
  for i in 0..count {
    let record = 4 + 8 * i;
    let platform = cmap.u16(record)?;
    let encoding = cmap.u16(record + 2)?;
    let offset = cmap.u32(record + 4)? as usize;
    let unicode = matches!((platform, encoding), (0, _) | (3, 1) | (3, 10));
    if !unicode {
      continue;
    }
    let found = match cmap.u16(offset)? {
      12 => (2, Cmap::Groups(offset)),
      4 => (1, Cmap::Segments(offset)),
      _ => continue,
    };
    if best.is_none_or(|(rank, _)| found.0 > rank) {
      best = Some(found);
    }
  }
  best.map(|(_, c)| c)
}

fn lookup_segments(
  cmap: Data<'_>,
  at: usize,
  c: u32,
) -> Option<u16> {
  if c > 0xffff {
    return None;
  }
  let c = c as u16;
  let segs = cmap.u16(at + 6)? as usize / 2;
  let ends = at + 14;
  let starts = ends + 2 * segs + 2;
  let deltas = starts + 2 * segs;
  let ranges = deltas + 2 * segs;
  // Binary search for the first segment whose end is at least `c`.
  let (mut lo, mut hi) = (0, segs);
  while lo < hi {
    let mid = (lo + hi) / 2;
    if cmap.u16(ends + 2 * mid)? < c {
      lo = mid + 1
    } else {
      hi = mid
    }
  }
  if lo == segs {
    return None;
  }
  let start = cmap.u16(starts + 2 * lo)?;
  if c < start {
    return None;
  }
  let delta = cmap.u16(deltas + 2 * lo)?;
  let range = cmap.u16(ranges + 2 * lo)? as usize;
  if range == 0 {
    return Some(c.wrapping_add(delta));
  }
  let glyph = cmap.u16(ranges + 2 * lo + range + 2 * (c - start) as usize)?;
  (glyph != 0).then(|| glyph.wrapping_add(delta))
}

fn lookup_groups(
  cmap: Data<'_>,
  at: usize,
  c: u32,
) -> Option<u16> {
  let groups = cmap.u32(at + 12)? as usize;
  let (mut lo, mut hi) = (0, groups);
  while lo < hi {
    let mid = (lo + hi) / 2;
    let g = at + 16 + 12 * mid;
    let (start, end) = (cmap.u32(g)?, cmap.u32(g + 4)?);
    if c < start {
      hi = mid;
    } else if c > end {
      lo = mid + 1;
    } else {
      return u16::try_from(cmap.u32(g + 8)? + (c - start)).ok();
    }
  }
  None
}

/// The family name from the `name` table: Windows English first.
fn family_name(name: Data<'_>) -> Option<String> {
  let count = name.u16(2)? as usize;
  let strings = name.u16(4)? as usize;
  let mut fallback = None;
  for i in 0..count {
    let r = 6 + 12 * i;
    let (platform, language, id) =
      (name.u16(r)?, name.u16(r + 4)?, name.u16(r + 6)?);
    if id != 1 {
      continue;
    }
    let (len, offset) = (name.u16(r + 8)? as usize, name.u16(r + 10)? as usize);
    let bytes = name.slice(strings + offset, len)?.0;
    let text = if platform == 3 || platform == 0 {
      let units: Vec<u16> = bytes
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
      String::from_utf16_lossy(&units)
    } else {
      bytes.iter().map(|&b| b as char).collect()
    };
    if platform == 3 && language == 0x0409 {
      return Some(text);
    }
    fallback.get_or_insert(text);
  }
  fallback
}
