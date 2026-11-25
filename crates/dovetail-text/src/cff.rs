//! CFF outlines (Compact Font Format, version 1), as in OpenType fonts
//! whose file starts with `OTTO`. Many fonts on Linux use them, among them
//! the large CJK fonts, which are CID-keyed: their glyphs are split into
//! groups, each with its own private data.
//!
//! A glyph is a Type 2 charstring: a small program of numbers and
//! operators that draws lines and cubic curves, with subroutines shared by
//! the whole font (global) or by a group of glyphs (local). Hints are
//! skipped: glyphs are drawn without hinting, as TrueType glyphs are.

use crate::outline::{Outline, Point, Segment};
use crate::read::Data;

/// An INDEX: a list of byte strings, as positions in the CFF table.
#[derive(Clone, Copy, Debug, Default)]
struct Index {
  count: usize,
  off_size: usize,
  /// Where the offsets start.
  offsets: usize,
  /// Where the data starts, minus one: offsets count from 1.
  data: usize,
  /// The first byte after the INDEX.
  end: usize,
}

impl Index {
  fn read(
    d: Data<'_>,
    at: usize,
  ) -> Option<Index> {
    let count = d.u16(at)? as usize;
    if count == 0 {
      return Some(Index { end: at + 2, ..Index::default() });
    }
    let off_size = d.u8(at + 2)? as usize;
    if !(1..=4).contains(&off_size) {
      return None;
    }
    let offsets = at + 3;
    let data = offsets + (count + 1) * off_size - 1;
    let index = Index { count, off_size, offsets, data, end: 0 };
    let last = index.offset(d, count)?;
    Some(Index { end: data + last, ..index })
  }

  fn offset(
    &self,
    d: Data<'_>,
    i: usize,
  ) -> Option<usize> {
    let at = self.offsets + i * self.off_size;
    let mut value = 0usize;
    for k in 0..self.off_size {
      value = (value << 8) | d.u8(at + k)? as usize;
    }
    Some(value)
  }

  /// Item `i`, as its start and length in the CFF table.
  fn get(
    &self,
    d: Data<'_>,
    i: usize,
  ) -> Option<(usize, usize)> {
    if i >= self.count {
      return None;
    }
    let (start, end) = (self.offset(d, i)?, self.offset(d, i + 1)?);
    if start == 0 || end < start {
      return None;
    }
    Some((self.data + start, end - start))
  }
}

/// The private data of a font, or of one group in a CID-keyed font.
#[derive(Clone, Copy, Debug, Default)]
struct Private {
  subrs: Index,
}

/// Which group each glyph of a CID-keyed font is in.
#[derive(Clone, Copy, Debug)]
enum FdSelect {
  /// Format 0: one byte for each glyph, from this position.
  Bytes(usize),
  /// Format 3: ranges, from this position.
  Ranges(usize),
}

/// A CFF table, read enough to draw its glyphs.
#[derive(Clone, Debug)]
pub(crate) struct Cff {
  charstrings: Index,
  global_subrs: Index,
  /// One for an ordinary font; one per group for a CID-keyed font.
  privates: Vec<Private>,
  fd_select: Option<FdSelect>,
}

/// The DICT operators used, with escaped ones as 1200 + the second byte.
const CHARSTRINGS: u16 = 17;
const PRIVATE: u16 = 18;
const SUBRS: u16 = 19;
const FD_ARRAY: u16 = 1236;
const FD_SELECT: u16 = 1237;

/// Reads a DICT: each operator with the operands before it.
fn dict(
  d: Data<'_>,
  start: usize,
  len: usize,
) -> Option<Vec<(u16, Vec<f64>)>> {
  let mut entries = Vec::new();
  let mut operands = Vec::new();
  let mut at = start;
  let end = start.checked_add(len)?;
  while at < end {
    let b0 = d.u8(at)?;
    at += 1;
    match b0 {
      0..=11 | 13..=21 => {
        entries.push((u16::from(b0), std::mem::take(&mut operands)));
      },
      12 => {
        let b1 = d.u8(at)?;
        at += 1;
        entries.push((1200 + u16::from(b1), std::mem::take(&mut operands)));
      },
      28 => {
        operands.push(f64::from(d.i16(at)?));
        at += 2;
      },
      29 => {
        operands.push(f64::from(d.u32(at)? as i32));
        at += 4;
      },
      30 => {
        // A real number in nibbles; only its length matters here.
        loop {
          let b = d.u8(at)?;
          at += 1;
          if b & 0x0f == 0x0f || b >> 4 == 0x0f {
            break;
          }
        }
        operands.push(0.0);
      },
      32..=246 => operands.push(f64::from(b0) - 139.0),
      247..=250 => {
        let b1 = d.u8(at)?;
        at += 1;
        operands.push(f64::from(b0 - 247) * 256.0 + f64::from(b1) + 108.0);
      },
      251..=254 => {
        let b1 = d.u8(at)?;
        at += 1;
        operands.push(-f64::from(b0 - 251) * 256.0 - f64::from(b1) - 108.0);
      },
      _ => return None,
    }
  }
  Some(entries)
}

fn find(
  entries: &[(u16, Vec<f64>)],
  op: u16,
) -> Option<&[f64]> {
  entries
    .iter()
    .find(|(o, _)| *o == op)
    .map(|(_, v)| v.as_slice())
}

/// Reads a Private DICT given as (size, offset) operands.
fn private(
  d: Data<'_>,
  operands: Option<&[f64]>,
) -> Option<Private> {
  let Some(&[size, offset]) = operands else {
    return Some(Private::default());
  };
  let (size, offset) = (size as usize, offset as usize);
  let entries = dict(d, offset, size)?;
  let subrs = match find(&entries, SUBRS) {
    Some(&[relative]) => Index::read(d, offset + relative as usize)?,
    _ => Index::default(),
  };
  Some(Private { subrs })
}

impl Cff {
  /// Reads the parts of a CFF table needed to draw glyphs.
  pub fn read(d: Data<'_>) -> Option<Cff> {
    if d.u8(0)? != 1 {
      return None;
    }
    let header_size = d.u8(2)? as usize;
    let names = Index::read(d, header_size)?;
    let top_dicts = Index::read(d, names.end)?;
    let strings = Index::read(d, top_dicts.end)?;
    let global_subrs = Index::read(d, strings.end)?;
    let (top_start, top_len) = top_dicts.get(d, 0)?;
    let top = dict(d, top_start, top_len)?;
    let charstrings = match find(&top, CHARSTRINGS) {
      Some(&[offset]) => Index::read(d, offset as usize)?,
      _ => return None,
    };

    let (privates, fd_select) = match find(&top, FD_ARRAY) {
      // CID-keyed: a Font DICT for each group, each with its Private DICT.
      Some(&[fd_array]) => {
        let fds = Index::read(d, fd_array as usize)?;
        let mut privates = Vec::with_capacity(fds.count);
        for i in 0..fds.count {
          let (start, len) = fds.get(d, i)?;
          let fd = dict(d, start, len)?;
          privates.push(private(d, find(&fd, PRIVATE))?);
        }
        let at = match find(&top, FD_SELECT) {
          Some(&[at]) => at as usize,
          _ => return None,
        };
        let select = match d.u8(at)? {
          0 => FdSelect::Bytes(at + 1),
          3 => FdSelect::Ranges(at + 1),
          _ => return None,
        };
        (privates, Some(select))
      },
      _ => (vec![private(d, find(&top, PRIVATE))?], None),
    };
    Some(Cff {
      charstrings,
      global_subrs,
      privates,
      fd_select,
    })
  }

  /// The group a glyph is in, in a CID-keyed font; 0 otherwise.
  fn group(
    &self,
    d: Data<'_>,
    glyph: u16,
  ) -> Option<usize> {
    match self.fd_select? {
      FdSelect::Bytes(at) => d.u8(at + glyph as usize).map(usize::from),
      FdSelect::Ranges(at) => {
        let count = d.u16(at)? as usize;
        // Ranges are sorted; each runs to the next one's first glyph.
        for i in 0..count {
          let range = at + 2 + 3 * i;
          let first = d.u16(range)?;
          let next = d.u16(range + 3)?;
          if glyph >= first && glyph < next {
            return d.u8(range + 2).map(usize::from);
          }
        }
        None
      },
    }
  }

  /// The outline of `glyph`, in font units with y up. Empty when it cannot
  /// be read.
  pub fn outline(
    &self,
    d: Data<'_>,
    glyph: u16,
  ) -> Outline {
    let group = if self.fd_select.is_some() {
      self.group(d, glyph)
    } else {
      Some(0)
    };
    let local = group
      .and_then(|g| self.privates.get(g))
      .map_or(Index::default(), |p| p.subrs);
    let mut run = Run {
      d,
      global: self.global_subrs,
      local,
      stack: Vec::with_capacity(48),
      stems: 0,
      width_done: false,
      at: Point::default(),
      start: None,
      outline: Outline::default(),
      steps: 0,
    };
    let ok = self
      .charstrings
      .get(d, glyph as usize)
      .and_then(|(start, len)| run.charstring(start, len, 0))
      .is_some();
    if !ok {
      // A charstring with bad data draws nothing rather than a broken
      // shape.
      return Outline::default();
    }
    run.close();
    let mut outline = run.outline;
    outline.fit_bounds();
    outline
  }
}

/// How deep subroutines may call each other, as in the CFF specification.
const MAX_DEPTH: usize = 10;
/// A limit on operators for one glyph, so bad data cannot run for long.
const MAX_STEPS: usize = 100_000;
/// The stack limit of Type 2 charstrings, with room to spare.
const MAX_STACK: usize = 96;

/// The state of drawing one glyph.
struct Run<'a> {
  d: Data<'a>,
  global: Index,
  local: Index,
  stack: Vec<f32>,
  /// Hint stems seen, which set how many bytes a hint mask has.
  stems: usize,
  /// The optional width at the start has been dealt with.
  width_done: bool,
  at: Point,
  /// Where the open contour started, if one is open.
  start: Option<Point>,
  outline: Outline,
  steps: usize,
}

/// Ends the charstring: `endchar`, as opposed to running off the end.
struct End;

impl Run<'_> {
  /// The subroutine number bias for an INDEX of `count` items.
  fn bias(count: usize) -> i32 {
    if count < 1240 {
      107
    } else if count < 33900 {
      1131
    } else {
      32768
    }
  }

  /// Runs a glyph's charstring. `None` when the data is bad.
  fn charstring(
    &mut self,
    start: usize,
    len: usize,
    depth: usize,
  ) -> Option<()> {
    match self.exec(start, len, depth) {
      Ok(()) => Some(()),
      Err(Some(End)) => Some(()),
      Err(None) => None,
    }
  }

  fn move_to(
    &mut self,
    p: Point,
  ) {
    self.close();
    self.at = p;
    self.start = Some(p);
  }

  fn line_to(
    &mut self,
    p: Point,
  ) {
    if self.start.is_none() {
      self.start = Some(self.at);
    }
    self
      .outline
      .segments
      .push(Segment::Line(self.at, p));
    self.at = p;
  }

  fn curve_to(
    &mut self,
    c1: Point,
    c2: Point,
    p: Point,
  ) {
    if self.start.is_none() {
      self.start = Some(self.at);
    }
    self
      .outline
      .segments
      .push(Segment::Cubic(self.at, c1, c2, p));
    self.at = p;
  }

  /// Closes the open contour with a line back to its start.
  fn close(&mut self) {
    if let Some(start) = self.start.take()
      && start != self.at
    {
      self
        .outline
        .segments
        .push(Segment::Line(self.at, start));
    }
  }

  /// Runs the operators in one charstring or subroutine.
  fn exec(
    &mut self,
    start: usize,
    len: usize,
    depth: usize,
  ) -> Result<(), Option<End>> {
    if depth > MAX_DEPTH {
      return Err(None);
    }
    let d = self.d;
    let end = start + len;
    let mut i = start;
    while i < end {
      self.steps += 1;
      if self.steps > MAX_STEPS {
        return Err(None);
      }
      let b0 = d.u8(i).ok_or(None)?;
      i += 1;
      // Operands.
      let number = match b0 {
        28 => {
          let v = d.i16(i).ok_or(None)?;
          i += 2;
          Some(f32::from(v))
        },
        32..=246 => Some(f32::from(b0) - 139.0),
        247..=250 => {
          let b1 = d.u8(i).ok_or(None)?;
          i += 1;
          Some(f32::from(b0 - 247) * 256.0 + f32::from(b1) + 108.0)
        },
        251..=254 => {
          let b1 = d.u8(i).ok_or(None)?;
          i += 1;
          Some(-f32::from(b0 - 251) * 256.0 - f32::from(b1) - 108.0)
        },
        255 => {
          let v = d.u32(i).ok_or(None)? as i32;
          i += 4;
          Some(v as f32 / 65536.0)
        },
        _ => None,
      };
      if let Some(n) = number {
        if self.stack.len() >= MAX_STACK {
          return Err(None);
        }
        self.stack.push(n);
        continue;
      }

      let s = std::mem::take(&mut self.stack);
      let mut s: &[f32] = &s;
      match b0 {
        // hstem, vstem, hstemhm, vstemhm: hints, only counted.
        1 | 3 | 18 | 23 => {
          if !self.width_done {
            self.width_done = true;
            if s.len() % 2 == 1 {
              s = &s[1..];
            }
          }
          self.stems += s.len() / 2;
        },
        // hintmask, cntrmask: stems given just before count too, then the
        // mask bytes are skipped.
        19 | 20 => {
          if !self.width_done {
            self.width_done = true;
            if s.len() % 2 == 1 {
              s = &s[1..];
            }
          }
          self.stems += s.len() / 2;
          i += self.stems.div_ceil(8);
        },
        // rmoveto
        21 => {
          if !self.width_done {
            self.width_done = true;
            if s.len() > 2 {
              s = &s[1..];
            }
          }
          let [dx, dy] = last::<2>(s)?;
          let p = rel(self.at, dx, dy);
          self.move_to(p);
        },
        // hmoveto, vmoveto
        22 | 4 => {
          if !self.width_done {
            self.width_done = true;
            if s.len() > 1 {
              s = &s[1..];
            }
          }
          let [delta] = last::<1>(s)?;
          let p = if b0 == 22 {
            rel(self.at, delta, 0.0)
          } else {
            rel(self.at, 0.0, delta)
          };
          self.move_to(p);
        },
        // rlineto
        5 => {
          for pair in s.as_chunks::<2>().0 {
            let p = rel(self.at, pair[0], pair[1]);
            self.line_to(p);
          }
        },
        // hlineto, vlineto: lines that turn at each step.
        6 | 7 => {
          let mut horizontal = b0 == 6;
          for &v in s {
            let p = if horizontal {
              rel(self.at, v, 0.0)
            } else {
              rel(self.at, 0.0, v)
            };
            self.line_to(p);
            horizontal = !horizontal;
          }
        },
        // rrcurveto
        8 => {
          for c in s.as_chunks::<6>().0 {
            self.curve(c);
          }
        },
        // rcurveline: curves, then a line.
        24 => {
          if s.len() < 8 {
            return Err(None);
          }
          let curves = (s.len() - 2) / 6;
          for c in s[..curves * 6].as_chunks::<6>().0 {
            self.curve(c);
          }
          let rest = &s[curves * 6..];
          let p = rel(self.at, rest[0], rest[1]);
          self.line_to(p);
        },
        // rlinecurve: lines, then a curve.
        25 => {
          if s.len() < 8 {
            return Err(None);
          }
          let lines = (s.len() - 6) / 2;
          for pair in s[..lines * 2].as_chunks::<2>().0 {
            let p = rel(self.at, pair[0], pair[1]);
            self.line_to(p);
          }
          self.curve(&s[lines * 2..]);
        },
        // vvcurveto: curves that start and end vertical.
        26 => {
          let mut dx1 = 0.0;
          if s.len() % 4 == 1 {
            dx1 = s[0];
            s = &s[1..];
          }
          for c in s.as_chunks::<4>().0 {
            let c1 = rel(self.at, dx1, c[0]);
            let c2 = rel(c1, c[1], c[2]);
            let p = rel(c2, 0.0, c[3]);
            self.curve_to(c1, c2, p);
            dx1 = 0.0;
          }
        },
        // hhcurveto: curves that start and end horizontal.
        27 => {
          let mut dy1 = 0.0;
          if s.len() % 4 == 1 {
            dy1 = s[0];
            s = &s[1..];
          }
          for c in s.as_chunks::<4>().0 {
            let c1 = rel(self.at, c[0], dy1);
            let c2 = rel(c1, c[1], c[2]);
            let p = rel(c2, c[3], 0.0);
            self.curve_to(c1, c2, p);
            dy1 = 0.0;
          }
        },
        // vhcurveto, hvcurveto: curves that turn between vertical and
        // horizontal; the last may end with an extra delta.
        30 | 31 => {
          let mut vertical = b0 == 30;
          while s.len() >= 4 {
            let extra = if s.len() == 5 { s[4] } else { 0.0 };
            let (c1, c2, p);
            if vertical {
              c1 = rel(self.at, 0.0, s[0]);
              c2 = rel(c1, s[1], s[2]);
              p = rel(c2, s[3], extra);
            } else {
              c1 = rel(self.at, s[0], 0.0);
              c2 = rel(c1, s[1], s[2]);
              p = rel(c2, extra, s[3]);
            }
            self.curve_to(c1, c2, p);
            s = &s[if s.len() == 5 { 5 } else { 4 }..];
            vertical = !vertical;
          }
        },
        // callsubr, callgsubr
        10 | 29 => {
          let (&number, rest) = s.split_last().ok_or(None)?;
          // The operands before the number stay for the subroutine.
          self.stack = rest.to_vec();
          let index = if b0 == 10 { self.local } else { self.global };
          let n = number as i32 + Self::bias(index.count);
          let (sub_start, sub_len) = index
            .get(self.d, usize::try_from(n).map_err(|_| None)?)
            .ok_or(None)?;
          self.exec(sub_start, sub_len, depth + 1)?;
          continue;
        },
        // return: the stack goes back to the caller.
        11 => {
          self.stack = s.to_vec();
          return Ok(());
        },
        // endchar
        14 => {
          if !self.width_done {
            self.width_done = true;
          }
          self.close();
          return Err(Some(End));
        },
        12 => {
          let b1 = d.u8(i).ok_or(None)?;
          i += 1;
          self.escaped(b1, s)?;
        },
        _ => return Err(None),
      }
      // Whatever came first, a width can no longer appear.
      self.width_done = true;
    }
    Ok(())
  }

  /// One rrcurveto-style curve from six deltas.
  fn curve(
    &mut self,
    c: &[f32],
  ) {
    let c1 = rel(self.at, c[0], c[1]);
    let c2 = rel(c1, c[2], c[3]);
    let p = rel(c2, c[4], c[5]);
    self.curve_to(c1, c2, p);
  }

  /// The flex operators, which draw two curves; other escaped operators
  /// (arithmetic, rarely used) are skipped.
  fn escaped(
    &mut self,
    op: u8,
    s: &[f32],
  ) -> Result<(), Option<End>> {
    let start = self.at;
    match op {
      // flex
      35 => {
        let s = s.get(..12).ok_or(None)?;
        self.curve(&s[..6]);
        self.curve(&s[6..]);
      },
      // hflex
      34 => {
        let [dx1, dx2, dy2, dx3, dx4, dx5, dx6] = first::<7>(s)?;
        let c1 = rel(start, dx1, 0.0);
        let c2 = rel(c1, dx2, dy2);
        let p = rel(c2, dx3, 0.0);
        self.curve_to(c1, c2, p);
        let c3 = rel(p, dx4, 0.0);
        let c4 = Point::new(c3.x + dx5, start.y);
        let e = rel(c4, dx6, 0.0);
        self.curve_to(c3, c4, e);
      },
      // hflex1
      36 => {
        let [
          dx1,
          dy1,
          dx2,
          dy2,
          dx3,
          dx4,
          dx5,
          dy5,
          dx6,
        ] = first::<9>(s)?;
        let c1 = rel(start, dx1, dy1);
        let c2 = rel(c1, dx2, dy2);
        let p = rel(c2, dx3, 0.0);
        self.curve_to(c1, c2, p);
        let c3 = rel(p, dx4, 0.0);
        let c4 = rel(c3, dx5, dy5);
        let e = Point::new(c4.x + dx6, start.y);
        self.curve_to(c3, c4, e);
      },
      // flex1
      37 => {
        let v = first::<11>(s)?;
        let c1 = rel(start, v[0], v[1]);
        let c2 = rel(c1, v[2], v[3]);
        let p = rel(c2, v[4], v[5]);
        self.curve_to(c1, c2, p);
        let c3 = rel(p, v[6], v[7]);
        let c4 = rel(c3, v[8], v[9]);
        let (dx, dy) = (c4.x - start.x, c4.y - start.y);
        let e = if dx.abs() > dy.abs() {
          Point::new(c4.x + v[10], start.y)
        } else {
          Point::new(start.x, c4.y + v[10])
        };
        self.curve_to(c3, c4, e);
      },
      _ => {},
    }
    Ok(())
  }
}

/// The last `N` operands.
fn last<const N: usize>(s: &[f32]) -> Result<[f32; N], Option<End>> {
  s.get(s.len().checked_sub(N).ok_or(None)?..)
    .and_then(|t| t.try_into().ok())
    .ok_or(None)
}

/// The first `N` operands.
fn first<const N: usize>(s: &[f32]) -> Result<[f32; N], Option<End>> {
  s.get(..N)
    .and_then(|t| t.try_into().ok())
    .ok_or(None)
}

/// `from` moved by (`dx`, `dy`).
fn rel(
  from: Point,
  dx: f32,
  dy: f32,
) -> Point {
  Point::new(from.x + dx, from.y + dy)
}
