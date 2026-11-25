//! Kerning: how much closer or further apart two glyphs go, such as "A"
//! and "V". Read from the `GPOS` table's pair adjustment lookups (type 2)
//! for the `kern` feature, or, in older fonts without them, from the `kern`
//! table (format 0).
//!
//! Only the horizontal advance of the first glyph is used, which is how
//! fonts express Latin kerning.

use crate::read::Data;

/// Where a font's kerning data is, found once when the font is read.
#[derive(Clone, Debug, Default)]
pub(crate) enum Kerning {
  /// No kerning.
  #[default]
  None,
  /// `GPOS` pair adjustment: for each lookup, its subtables' offsets from
  /// the start of `GPOS`.
  Gpos(Vec<Vec<usize>>),
  /// `kern` format 0 subtables: the offset of each one's pairs (after its
  /// header) and how many there are, from the start of `kern`.
  Kern(Vec<(usize, usize)>),
}

const PAIR_ADJUSTMENT: u16 = 2;
const EXTENSION: u16 = 9;

impl Kerning {
  /// Finds the kerning in a font's `GPOS` and `kern` tables (either may be
  /// empty).
  pub fn find(
    gpos: Data<'_>,
    kern: Data<'_>,
  ) -> Kerning {
    if let Some(lookups) = gpos_lookups(gpos)
      && !lookups.is_empty()
    {
      return Kerning::Gpos(lookups);
    }
    match kern_subtables(kern) {
      Some(subtables) if !subtables.is_empty() => Kerning::Kern(subtables),
      _ => Kerning::None,
    }
  }

  /// The change to `left`'s advance when `right` follows it, in font
  /// units. Negative moves them closer.
  pub fn pair(
    &self,
    gpos: Data<'_>,
    kern: Data<'_>,
    left: u16,
    right: u16,
  ) -> i32 {
    match self {
      Kerning::None => 0,
      Kerning::Gpos(lookups) => lookups
        .iter()
        .map(|subtables| {
          // The first subtable that applies to the pair gives the value.
          subtables
            .iter()
            .find_map(|&at| pair_adjustment(gpos, at, left, right))
            .unwrap_or(0)
        })
        .sum(),
      Kerning::Kern(subtables) => subtables
        .iter()
        .filter_map(|&(at, count)| kern_pair(kern, at, count, left, right))
        .sum(),
    }
  }
}

/// The pair adjustment lookups used by the `kern` feature, in lookup
/// order, each as its subtables' offsets in `GPOS`.
fn gpos_lookups(gpos: Data<'_>) -> Option<Vec<Vec<usize>>> {
  if gpos.u16(0)? != 1 {
    return None;
  }
  let features = gpos.u16(6)? as usize;
  let lookup_list = gpos.u16(8)? as usize;

  // Every lookup any `kern` feature uses, whatever the script.
  let mut indexes = Vec::new();
  let count = gpos.u16(features)? as usize;
  for i in 0..count {
    let record = features + 2 + 6 * i;
    if &gpos.tag(record)? != b"kern" {
      continue;
    }
    let feature = features + gpos.u16(record + 4)? as usize;
    let lookups = gpos.u16(feature + 2)? as usize;
    for j in 0..lookups {
      indexes.push(gpos.u16(feature + 4 + 2 * j)?);
    }
  }
  indexes.sort_unstable();
  indexes.dedup();

  let lookup_count = gpos.u16(lookup_list)?;
  let mut result = Vec::new();
  for index in indexes {
    if index >= lookup_count {
      continue;
    }
    let lookup =
      lookup_list + gpos.u16(lookup_list + 2 + 2 * index as usize)? as usize;
    let kind = gpos.u16(lookup)?;
    let subtables = gpos.u16(lookup + 4)? as usize;
    let mut offsets = Vec::new();
    for k in 0..subtables {
      let at = lookup + gpos.u16(lookup + 6 + 2 * k)? as usize;
      match kind {
        PAIR_ADJUSTMENT => offsets.push(at),
        // An extension holds a subtable further away, with a 32-bit offset.
        EXTENSION if gpos.u16(at + 2)? == PAIR_ADJUSTMENT => {
          offsets.push(at + gpos.u32(at + 4)? as usize);
        },
        _ => {},
      }
    }
    if !offsets.is_empty() {
      result.push(offsets);
    }
  }
  Some(result)
}

/// The size of a value record with this format: two bytes per field.
fn value_size(format: u16) -> usize {
  2 * format.count_ones() as usize
}

/// The x advance in a value record, or 0 when it has none.
fn x_advance(
  d: Data<'_>,
  at: usize,
  format: u16,
) -> Option<i32> {
  if format & 0x0004 == 0 {
    return Some(0);
  }
  // X and Y placement come first, when present.
  let before = (format & 0x0003).count_ones() as usize;
  d.i16(at + 2 * before).map(i32::from)
}

/// The adjustment from one pair adjustment subtable, or `None` when it
/// does not apply to the pair.
fn pair_adjustment(
  d: Data<'_>,
  at: usize,
  left: u16,
  right: u16,
) -> Option<i32> {
  let format = d.u16(at)?;
  let coverage_index = coverage(d, at + d.u16(at + 2)? as usize, left)?;
  let format1 = d.u16(at + 4)?;
  let format2 = d.u16(at + 6)?;
  let (size1, size2) = (value_size(format1), value_size(format2));
  match format {
    1 => {
      // A list of second glyphs for each first glyph, sorted.
      let sets = d.u16(at + 8)? as usize;
      if coverage_index >= sets {
        return None;
      }
      let set = at + d.u16(at + 10 + 2 * coverage_index)? as usize;
      let count = d.u16(set)? as usize;
      let stride = 2 + size1 + size2;
      let (mut lo, mut hi) = (0, count);
      while lo < hi {
        let mid = (lo + hi) / 2;
        let record = set + 2 + mid * stride;
        let second = d.u16(record)?;
        match second.cmp(&right) {
          std::cmp::Ordering::Less => lo = mid + 1,
          std::cmp::Ordering::Greater => hi = mid,
          std::cmp::Ordering::Equal => {
            return x_advance(d, record + 2, format1);
          },
        }
      }
      None
    },
    2 => {
      // A table of values by the class of each glyph.
      let class1 = class(d, at + d.u16(at + 8)? as usize, left)? as usize;
      let class2 = class(d, at + d.u16(at + 10)? as usize, right)? as usize;
      let class1_count = d.u16(at + 12)? as usize;
      let class2_count = d.u16(at + 14)? as usize;
      if class1 >= class1_count || class2 >= class2_count {
        return Some(0);
      }
      let record = at + 16 + (class1 * class2_count + class2) * (size1 + size2);
      x_advance(d, record, format1)
    },
    _ => None,
  }
}

/// The glyph's position in a coverage table, or `None` when it is not in
/// it.
pub(crate) fn coverage(
  d: Data<'_>,
  at: usize,
  glyph: u16,
) -> Option<usize> {
  let count = d.u16(at + 2)? as usize;
  match d.u16(at)? {
    1 => {
      let (mut lo, mut hi) = (0, count);
      while lo < hi {
        let mid = (lo + hi) / 2;
        match d.u16(at + 4 + 2 * mid)?.cmp(&glyph) {
          std::cmp::Ordering::Less => lo = mid + 1,
          std::cmp::Ordering::Greater => hi = mid,
          std::cmp::Ordering::Equal => return Some(mid),
        }
      }
      None
    },
    2 => {
      let (mut lo, mut hi) = (0, count);
      while lo < hi {
        let mid = (lo + hi) / 2;
        let range = at + 4 + 6 * mid;
        let (start, end) = (d.u16(range)?, d.u16(range + 2)?);
        if glyph < start {
          hi = mid;
        } else if glyph > end {
          lo = mid + 1;
        } else {
          let first = d.u16(range + 4)? as usize;
          return Some(first + (glyph - start) as usize);
        }
      }
      None
    },
    _ => None,
  }
}

/// The glyph's class in a class definition table. Glyphs not listed are
/// class 0.
pub(crate) fn class(
  d: Data<'_>,
  at: usize,
  glyph: u16,
) -> Option<u16> {
  match d.u16(at)? {
    1 => {
      let start = d.u16(at + 2)?;
      let count = d.u16(at + 4)?;
      if glyph < start || glyph - start >= count {
        return Some(0);
      }
      d.u16(at + 6 + 2 * (glyph - start) as usize)
    },
    2 => {
      let count = d.u16(at + 2)? as usize;
      let (mut lo, mut hi) = (0, count);
      while lo < hi {
        let mid = (lo + hi) / 2;
        let range = at + 4 + 6 * mid;
        let (start, end) = (d.u16(range)?, d.u16(range + 2)?);
        if glyph < start {
          hi = mid;
        } else if glyph > end {
          lo = mid + 1;
        } else {
          return d.u16(range + 4);
        }
      }
      Some(0)
    },
    _ => Some(0),
  }
}

/// The horizontal format 0 subtables of a `kern` table (version 0), as
/// the offset of their pairs and how many there are.
fn kern_subtables(kern: Data<'_>) -> Option<Vec<(usize, usize)>> {
  if kern.u16(0)? != 0 {
    // Apple's version 1 tables are not read.
    return None;
  }
  let count = kern.u16(2)? as usize;
  let mut at = 4;
  let mut result = Vec::new();
  for _ in 0..count {
    let length = kern.u16(at + 2)? as usize;
    let coverage = kern.u16(at + 4)?;
    let format = coverage >> 8;
    // Horizontal, kerning values (not minimums), not across the line.
    if format == 0 && coverage & 0x0007 == 0x0001 {
      let pairs = kern.u16(at + 6)? as usize;
      result.push((at + 14, pairs));
    }
    if length < 6 {
      break;
    }
    at += length;
  }
  Some(result)
}

/// The value for a pair in a `kern` format 0 subtable, if listed.
fn kern_pair(
  kern: Data<'_>,
  at: usize,
  count: usize,
  left: u16,
  right: u16,
) -> Option<i32> {
  let key = (u32::from(left) << 16) | u32::from(right);
  let (mut lo, mut hi) = (0, count);
  while lo < hi {
    let mid = (lo + hi) / 2;
    let pair = at + 6 * mid;
    match kern.u32(pair)?.cmp(&key) {
      std::cmp::Ordering::Less => lo = mid + 1,
      std::cmp::Ordering::Greater => hi = mid,
      std::cmp::Ordering::Equal => {
        return kern.i16(pair + 4).map(i32::from);
      },
    }
  }
  None
}
