//! OpenType layout for complex scripts (M13): glyph substitution from
//! `GSUB` and mark positioning from `GPOS`, with glyph classes from
//! `GDEF`.
//!
//! - `GSUB` lookup types 1 (single), 2 (multiple), 4 (ligature), 5
//!   (context), 6 (chained context) and 7 (extension), with lookup flags
//!   that skip base glyphs, ligatures, marks, or marks of other classes.
//! - `GPOS` lookup types 4 (mark to base), 5 (mark to ligature) and 6 (mark
//!   to mark), and 9 (extension).
//!
//! Features are applied for one script's default language system. Each
//! glyph carries a mask of the features that apply to it, so `init`,
//! `medi`, `fina` and `isol` reach only the glyphs that need them.

use crate::font::Font;
use crate::kern::{class, coverage};
use crate::read::Data;

/// One glyph being shaped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Glyph {
  pub glyph: u16,
  /// The first character it shows, as an index into the run's characters.
  pub first: usize,
  /// One past the last character it shows.
  pub end: usize,
  /// The features that apply to it.
  pub mask: u32,
  /// For a mark inside or after a ligature: which ligature (counted from
  /// 1) and which of its components it goes with.
  pub ligature: u32,
  pub component: u16,
}

/// Where a mark goes: on glyph `base`, moved by (`dx`, `dy`) font units
/// from the base's pen position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Attachment {
  pub base: usize,
  pub dx: f32,
  pub dy: f32,
}

const RIGHT_TO_LEFT: u16 = 0x0001;
const IGNORE_BASE: u16 = 0x0002;
const IGNORE_LIGATURES: u16 = 0x0004;
const IGNORE_MARKS: u16 = 0x0008;
const USE_MARK_FILTERING_SET: u16 = 0x0010;
const MARK_ATTACHMENT_TYPE: u16 = 0xFF00;

/// The tables, and the lookups chosen for a script and its features.
pub(crate) struct Layout<'a> {
  table: Data<'a>,
  gdef: Data<'a>,
  /// Each lookup to apply, in lookup order, with the features' mask.
  lookups: Vec<(u16, u32)>,
  gsub: bool,
}

/// `GDEF` glyph class: 1 base, 2 ligature, 3 mark, 4 component.
pub(crate) fn glyph_class(
  gdef: Data<'_>,
  glyph: u16,
) -> u16 {
  let Some(at) = gdef.u16(4).filter(|&o| o != 0) else {
    return 0;
  };
  class(gdef, at as usize, glyph).unwrap_or(0)
}

fn mark_attach_class(
  gdef: Data<'_>,
  glyph: u16,
) -> u16 {
  let Some(at) = gdef.u16(10).filter(|&o| o != 0) else {
    return 0;
  };
  class(gdef, at as usize, glyph).unwrap_or(0)
}

/// True when `glyph` is in mark glyph set `set` (GDEF 1.2).
fn in_mark_set(
  gdef: Data<'_>,
  set: u16,
  glyph: u16,
) -> bool {
  if gdef.u16(2) < Some(2) {
    return false;
  }
  let Some(sets) = gdef.u16(12).filter(|&o| o != 0) else {
    return false;
  };
  let sets = sets as usize;
  if set >= gdef.u16(sets + 2).unwrap_or(0) {
    return false;
  }
  let Some(offset) = gdef.u32(sets + 4 + 4 * set as usize) else {
    return false;
  };
  coverage(gdef, sets + offset as usize, glyph).is_some()
}

impl<'a> Layout<'a> {
  /// The lookups `font`'s `GSUB` (or `GPOS`) uses for `script` and the
  /// features given with their mask bits. `None` when the font has no
  /// such table or script.
  pub fn new(
    font: &'a Font,
    gsub: bool,
    script: &[u8; 4],
    features: &[([u8; 4], u32)],
  ) -> Option<Layout<'a>> {
    let table = font.table(if gsub { b"GSUB" } else { b"GPOS" });
    let gdef = font.table(b"GDEF");
    if table.u16(0)? != 1 {
      return None;
    }
    let scripts = table.u16(4)? as usize;
    let feature_list = table.u16(6)? as usize;
    let count = table.u16(scripts)? as usize;
    let mut found = None;
    for wanted in [script, b"DFLT", b"latn"] {
      for i in 0..count {
        let record = scripts + 2 + 6 * i;
        if &table.tag(record)? == wanted {
          found = Some(scripts + table.u16(record + 4)? as usize);
          break;
        }
      }
      if found.is_some() {
        break;
      }
    }
    let script_table = found?;
    let lang = table.u16(script_table)? as usize;
    if lang == 0 {
      return None;
    }
    let lang = script_table + lang;
    let required = table.u16(lang + 2)?;
    let count = table.u16(lang + 4)? as usize;
    let mut indexes: Vec<u16> = (0..count)
      .filter_map(|i| table.u16(lang + 6 + 2 * i))
      .collect();
    if required != 0xFFFF {
      indexes.push(required);
    }
    let mut lookups: Vec<(u16, u32)> = Vec::new();
    let feature_count = table.u16(feature_list)?;
    for index in indexes {
      if index >= feature_count {
        continue;
      }
      let record = feature_list + 2 + 6 * index as usize;
      let tag = table.tag(record)?;
      let Some(&(_, mask)) = features.iter().find(|(t, _)| *t == tag) else {
        continue;
      };
      let feature = feature_list + table.u16(record + 4)? as usize;
      let n = table.u16(feature + 2)? as usize;
      for j in 0..n {
        let lookup = table.u16(feature + 4 + 2 * j)?;
        match lookups.iter_mut().find(|(l, _)| *l == lookup) {
          Some(entry) => entry.1 |= mask,
          None => lookups.push((lookup, mask)),
        }
      }
    }
    lookups.sort_unstable_by_key(|l| l.0);
    Some(Layout { table, gdef, lookups, gsub })
  }

  /// The offset of lookup `index`, its type, flag, mark filtering set and
  /// subtables (extensions opened).
  fn lookup(
    &self,
    index: u16,
  ) -> Option<(u16, u16, u16, Vec<usize>)> {
    let t = self.table;
    let list = t.u16(8)? as usize;
    if index >= t.u16(list)? {
      return None;
    }
    let at = list + t.u16(list + 2 + 2 * index as usize)? as usize;
    let mut kind = t.u16(at)?;
    let flag = t.u16(at + 2)?;
    let n = t.u16(at + 4)? as usize;
    let set = if flag & USE_MARK_FILTERING_SET != 0 {
      t.u16(at + 6 + 2 * n)?
    } else {
      0
    };
    let extension = if self.gsub { 7 } else { 9 };
    let mut subtables = Vec::with_capacity(n);
    for k in 0..n {
      let sub = at + t.u16(at + 6 + 2 * k)? as usize;
      if kind == extension {
        kind = t.u16(sub + 2)?;
        subtables.push(sub + t.u32(sub + 4)? as usize);
      } else {
        subtables.push(sub);
      }
    }
    Some((kind, flag, set, subtables))
  }

  /// True when a lookup with this flag skips `glyph`.
  fn skips(
    &self,
    flag: u16,
    set: u16,
    glyph: u16,
  ) -> bool {
    let class = glyph_class(self.gdef, glyph);
    match class {
      1 => flag & IGNORE_BASE != 0,
      2 => flag & IGNORE_LIGATURES != 0,
      3 => {
        if flag & IGNORE_MARKS != 0 {
          return true;
        }
        if flag & USE_MARK_FILTERING_SET != 0 {
          return !in_mark_set(self.gdef, set, glyph);
        }
        let wanted = (flag & MARK_ATTACHMENT_TYPE) >> 8;
        wanted != 0 && mark_attach_class(self.gdef, glyph) != wanted
      },
      _ => false,
    }
  }

  /// Applies the `GSUB` lookups to `glyphs`.
  pub fn substitute(
    &self,
    glyphs: &mut Vec<Glyph>,
  ) {
    for &(index, mask) in &self.lookups {
      let mut i = 0;
      while i < glyphs.len() {
        if glyphs[i].mask & mask != 0
          && let Some(next) = self.apply(index, glyphs, i, 0)
        {
          i = next.max(i + 1);
          continue;
        }
        i += 1;
      }
    }
  }

  /// The next glyph after `i` that the lookup does not skip.
  fn next(
    &self,
    glyphs: &[Glyph],
    mut i: usize,
    flag: u16,
    set: u16,
  ) -> Option<usize> {
    loop {
      i += 1;
      let g = glyphs.get(i)?;
      if !self.skips(flag, set, g.glyph) {
        return Some(i);
      }
    }
  }

  fn previous(
    &self,
    glyphs: &[Glyph],
    mut i: usize,
    flag: u16,
    set: u16,
  ) -> Option<usize> {
    loop {
      i = i.checked_sub(1)?;
      if !self.skips(flag, set, glyphs[i].glyph) {
        return Some(i);
      }
    }
  }

  /// Applies lookup `index` at glyph `i`. Returns where to go on, when it
  /// applied.
  fn apply(
    &self,
    index: u16,
    glyphs: &mut Vec<Glyph>,
    i: usize,
    depth: u8,
  ) -> Option<usize> {
    if depth > 8 {
      return None;
    }
    let (kind, flag, set, subtables) = self.lookup(index)?;
    if self.skips(flag, set, glyphs[i].glyph) {
      return None;
    }
    let t = self.table;
    let glyph = glyphs[i].glyph;
    for sub in subtables {
      let applied = match kind {
        1 => (|| {
          let cov = coverage(t, sub + t.u16(sub + 2)? as usize, glyph)?;
          let new = match t.u16(sub)? {
            1 => glyph.wrapping_add(t.u16(sub + 4)?),
            2 => t.u16(sub + 6 + 2 * cov)?,
            _ => return None,
          };
          glyphs[i].glyph = new;
          Some(i + 1)
        })(),
        2 => (|| {
          let cov = coverage(t, sub + t.u16(sub + 2)? as usize, glyph)?;
          let seq = sub + t.u16(sub + 6 + 2 * cov)? as usize;
          let n = t.u16(seq)? as usize;
          let new: Option<Vec<u16>> =
            (0..n).map(|k| t.u16(seq + 2 + 2 * k)).collect();
          let new = new?;
          if new.is_empty() {
            glyphs.remove(i);
            return Some(i);
          }
          let template = glyphs[i];
          glyphs.splice(
            i..=i,
            new
              .iter()
              .map(|&g| Glyph { glyph: g, ..template }),
          );
          Some(i + n)
        })(),
        4 => self.ligature(sub, glyphs, i, flag, set),
        5 | 6 => self.context(kind, sub, glyphs, i, flag, set, depth),
        _ => None,
      };
      if applied.is_some() {
        return applied;
      }
    }
    None
  }

  fn ligature(
    &self,
    sub: usize,
    glyphs: &mut Vec<Glyph>,
    i: usize,
    flag: u16,
    set: u16,
  ) -> Option<usize> {
    let t = self.table;
    let cov = coverage(t, sub + t.u16(sub + 2)? as usize, glyphs[i].glyph)?;
    let lig_set = sub + t.u16(sub + 6 + 2 * cov)? as usize;
    let count = t.u16(lig_set)? as usize;
    'ligatures: for l in 0..count {
      let lig = lig_set + t.u16(lig_set + 2 + 2 * l)? as usize;
      let new = t.u16(lig)?;
      let parts = t.u16(lig + 2)? as usize;
      let mut at = vec![i];
      for p in 1..parts {
        let Some(next) = self.next(glyphs, *at.last().unwrap(), flag, set)
        else {
          continue 'ligatures;
        };
        if glyphs[next].glyph != t.u16(lig + 4 + 2 * (p - 1))? {
          continue 'ligatures;
        }
        at.push(next);
      }
      // Marks between the parts stay, and go with the part before them.
      let id = glyphs
        .iter()
        .map(|g| g.ligature)
        .max()
        .unwrap_or(0)
        + 1;
      let last = *at.last().unwrap();
      for (part, w) in at.windows(2).enumerate() {
        for g in &mut glyphs[w[0] + 1..w[1]] {
          g.ligature = id;
          g.component = part as u16;
        }
      }
      let end = glyphs[last].end.max(glyphs[i].end);
      glyphs[i].glyph = new;
      glyphs[i].end = end;
      glyphs[i].ligature = id;
      glyphs[i].component = parts as u16;
      for &k in at[1..].iter().rev() {
        glyphs.remove(k);
      }
      return Some(i + 1);
    }
    None
  }

  /// Context (5) and chained context (6) lookups, in all three formats.
  #[allow(clippy::too_many_arguments)]
  fn context(
    &self,
    kind: u16,
    sub: usize,
    glyphs: &mut Vec<Glyph>,
    i: usize,
    flag: u16,
    set: u16,
    depth: u8,
  ) -> Option<usize> {
    let t = self.table;
    let format = t.u16(sub)?;
    let glyph = glyphs[i].glyph;
    // Each rule: backtrack, input (after the first), lookahead, and the
    // lookups to apply, as matchers on glyphs.
    enum Match {
      Glyph(u16),
      Class(usize, u16),
      Coverage(usize),
    }
    let matches = |m: &Match, g: u16| match *m {
      Match::Glyph(x) => g == x,
      Match::Class(def, c) => class(t, def, g).unwrap_or(0) == c,
      Match::Coverage(c) => coverage(t, c, g).is_some(),
    };
    // The last part is the count of lookup records and where they start.
    let mut rules: Vec<Rule<Match>> = Vec::new();
    match (kind, format) {
      (5, 1) | (6, 1) => {
        let cov = coverage(t, sub + t.u16(sub + 2)? as usize, glyph)?;
        let sets = t.u16(sub + 4)? as usize;
        if cov >= sets {
          return None;
        }
        let rs = sub + t.u16(sub + 6 + 2 * cov)? as usize;
        for r in 0..t.u16(rs)? as usize {
          let rule = rs + t.u16(rs + 2 + 2 * r)? as usize;
          rules.push(read_rule(t, kind, rule, Match::Glyph)?);
        }
      },
      (5, 2) => {
        coverage(t, sub + t.u16(sub + 2)? as usize, glyph)?;
        let def = sub + t.u16(sub + 4)? as usize;
        let c = class(t, def, glyph).unwrap_or(0) as usize;
        if c >= t.u16(sub + 6)? as usize {
          return None;
        }
        let offset = t.u16(sub + 8 + 2 * c)? as usize;
        if offset == 0 {
          return None;
        }
        let rs = sub + offset;
        for r in 0..t.u16(rs)? as usize {
          let rule = rs + t.u16(rs + 2 + 2 * r)? as usize;
          rules.push(read_rule(t, kind, rule, |c| Match::Class(def, c))?);
        }
      },
      (6, 2) => {
        coverage(t, sub + t.u16(sub + 2)? as usize, glyph)?;
        let back = sub + t.u16(sub + 4)? as usize;
        let input = sub + t.u16(sub + 6)? as usize;
        let ahead = sub + t.u16(sub + 8)? as usize;
        let c = class(t, input, glyph).unwrap_or(0) as usize;
        if c >= t.u16(sub + 10)? as usize {
          return None;
        }
        let offset = t.u16(sub + 12 + 2 * c)? as usize;
        if offset == 0 {
          return None;
        }
        let rs = sub + offset;
        for r in 0..t.u16(rs)? as usize {
          let rule = rs + t.u16(rs + 2 + 2 * r)? as usize;
          let (b, inp, a, records) = read_chain(t, rule)?;
          rules.push((
            b.into_iter()
              .map(|v| Match::Class(back, v))
              .collect(),
            inp
              .into_iter()
              .map(|v| Match::Class(input, v))
              .collect(),
            a.into_iter()
              .map(|v| Match::Class(ahead, v))
              .collect(),
            (t.u16(records)? as usize, records + 2),
          ));
        }
      },
      (5, 3) => {
        let n = t.u16(sub + 2)? as usize;
        let first = sub + t.u16(sub + 6)? as usize;
        coverage(t, first, glyph)?;
        let input = (1..n)
          .map(|k| {
            Some(Match::Coverage(sub + t.u16(sub + 6 + 2 * k)? as usize))
          })
          .collect::<Option<Vec<_>>>()?;
        rules.push((
          Vec::new(),
          input,
          Vec::new(),
          (t.u16(sub + 4)? as usize, sub + 6 + 2 * n),
        ));
      },
      (6, 3) => {
        let nb = t.u16(sub + 2)? as usize;
        let back = (0..nb)
          .map(|k| {
            Some(Match::Coverage(sub + t.u16(sub + 4 + 2 * k)? as usize))
          })
          .collect::<Option<Vec<_>>>()?;
        let at = sub + 4 + 2 * nb;
        let ni = t.u16(at)? as usize;
        if ni == 0 {
          return None;
        }
        coverage(t, sub + t.u16(at + 2)? as usize, glyph)?;
        let input = (1..ni)
          .map(|k| Some(Match::Coverage(sub + t.u16(at + 2 + 2 * k)? as usize)))
          .collect::<Option<Vec<_>>>()?;
        let at = at + 2 + 2 * ni;
        let na = t.u16(at)? as usize;
        let ahead = (0..na)
          .map(|k| Some(Match::Coverage(sub + t.u16(at + 2 + 2 * k)? as usize)))
          .collect::<Option<Vec<_>>>()?;
        let count_at = at + 2 + 2 * na;
        rules.push((
          back,
          input,
          ahead,
          (t.u16(count_at)? as usize, count_at + 2),
        ));
      },
      _ => return None,
    }
    'rules: for (back, input, ahead, records) in rules {
      let mut positions = vec![i];
      for m in &input {
        match self.next(glyphs, *positions.last().unwrap(), flag, set) {
          Some(n) if matches(m, glyphs[n].glyph) => positions.push(n),
          _ => continue 'rules,
        }
      }
      let mut at = i;
      for m in &back {
        match self.previous(glyphs, at, flag, set) {
          Some(p) if matches(m, glyphs[p].glyph) => at = p,
          _ => continue 'rules,
        }
      }
      let mut at = *positions.last().unwrap();
      for m in &ahead {
        match self.next(glyphs, at, flag, set) {
          Some(n) if matches(m, glyphs[n].glyph) => at = n,
          _ => continue 'rules,
        }
      }
      let (count, records) = records;
      let mut end = *positions.last().unwrap() as isize + 1;
      for r in 0..count {
        let seq = t.u16(records + 4 * r)? as usize;
        let lookup = t.u16(records + 2 + 4 * r)?;
        let Some(&pos) = positions.get(seq) else {
          continue;
        };
        if pos >= glyphs.len() {
          continue;
        }
        let before = glyphs.len() as isize;
        self.apply(lookup, glyphs, pos, depth + 1);
        let change = glyphs.len() as isize - before;
        if change != 0 {
          // Later positions move with what was added or removed.
          for p in positions.iter_mut().filter(|p| **p > pos) {
            *p = (*p as isize + change).max(pos as isize) as usize;
          }
          end += change;
        }
      }
      return Some(end.max(i as isize + 1) as usize);
    }
    None
  }

  /// Applies the `GPOS` mark lookups: where each mark goes.
  pub fn position_marks(
    &self,
    glyphs: &[Glyph],
  ) -> Vec<Option<Attachment>> {
    let mut out = vec![None; glyphs.len()];
    for &(index, mask) in &self.lookups {
      let Some((kind, flag, set, subtables)) = self.lookup(index) else {
        continue;
      };
      if !(4..=6).contains(&kind) {
        continue;
      }
      for i in 0..glyphs.len() {
        if glyphs[i].mask & mask == 0
          || glyph_class(self.gdef, glyphs[i].glyph) != 3
        {
          continue;
        }
        if self.skips(flag, set, glyphs[i].glyph) {
          continue;
        }
        for &sub in &subtables {
          if let Some(a) = self.mark(kind, sub, glyphs, i) {
            out[i] = Some(a);
            break;
          }
        }
      }
    }
    out
  }

  fn anchor(
    &self,
    at: usize,
  ) -> Option<(f32, f32)> {
    let t = self.table;
    Some((f32::from(t.i16(at + 2)?), f32::from(t.i16(at + 4)?)))
  }

  fn mark(
    &self,
    kind: u16,
    sub: usize,
    glyphs: &[Glyph],
    i: usize,
  ) -> Option<Attachment> {
    let t = self.table;
    let mark_cov =
      coverage(t, sub + t.u16(sub + 2)? as usize, glyphs[i].glyph)?;
    // The glyph it goes on: the previous base or ligature for types 4 and
    // 5, and the mark just before it for type 6.
    let base = match kind {
      6 => {
        let p = i.checked_sub(1)?;
        (glyph_class(self.gdef, glyphs[p].glyph) == 3
          && glyphs[p].ligature == glyphs[i].ligature
          && glyphs[p].component == glyphs[i].component)
          .then_some(p)?
      },
      _ => (0..i)
        .rev()
        .find(|&p| glyph_class(self.gdef, glyphs[p].glyph) != 3)?,
    };
    let base_cov =
      coverage(t, sub + t.u16(sub + 4)? as usize, glyphs[base].glyph)?;
    let classes = t.u16(sub + 6)? as usize;
    let marks = sub + t.u16(sub + 8)? as usize;
    let bases = sub + t.u16(sub + 10)? as usize;
    let record = marks + 2 + 4 * mark_cov;
    let mark_class = t.u16(record)? as usize;
    let mark_anchor = self.anchor(marks + t.u16(record + 2)? as usize)?;
    if mark_class >= classes {
      return None;
    }
    let base_anchor_at = match kind {
      5 => {
        let attach = bases + t.u16(bases + 2 + 2 * base_cov)? as usize;
        let components = t.u16(attach)? as usize;
        if components == 0 {
          return None;
        }
        let component = if glyphs[i].ligature == glyphs[base].ligature
          && glyphs[i].ligature != 0
        {
          (glyphs[i].component as usize).min(components - 1)
        } else {
          components - 1
        };
        let offset =
          t.u16(attach + 2 + 2 * (component * classes + mark_class))?;
        if offset == 0 {
          return None;
        }
        attach + offset as usize
      },
      _ => {
        let offset =
          t.u16(bases + 2 + 2 * (base_cov * classes + mark_class))?;
        if offset == 0 {
          return None;
        }
        bases + offset as usize
      },
    };
    let base_anchor = self.anchor(base_anchor_at)?;
    Some(Attachment {
      base,
      dx: base_anchor.0 - mark_anchor.0,
      dy: base_anchor.1 - mark_anchor.1,
    })
  }
}

/// A rule of a context lookup: backtrack, input after the first glyph,
/// lookahead, and the count of lookup records and where they start.
type Rule<M> = (Vec<M>, Vec<M>, Vec<M>, (usize, usize));

/// A context rule (format 1 or 2) or chained rule (format 1): its
/// backtrack, input after the first glyph, and lookahead as values, and
/// the count of lookup records and where they start.
fn read_rule<M>(
  t: Data<'_>,
  kind: u16,
  rule: usize,
  make: impl Fn(u16) -> M,
) -> Option<Rule<M>> {
  if kind == 6 {
    let (b, i, a, count_at) = read_chain(t, rule)?;
    return Some((
      b.into_iter().map(&make).collect(),
      i.into_iter().map(&make).collect(),
      a.into_iter().map(&make).collect(),
      (t.u16(count_at)? as usize, count_at + 2),
    ));
  }
  let n = t.u16(rule)? as usize;
  let count = t.u16(rule + 2)? as usize;
  let input = (1..n)
    .map(|k| t.u16(rule + 4 + 2 * (k - 1)).map(&make))
    .collect::<Option<Vec<_>>>()?;
  Some((
    Vec::new(),
    input,
    Vec::new(),
    (count, rule + 4 + 2 * n.saturating_sub(1)),
  ))
}

/// A chained rule: backtrack, input (after the first) and lookahead
/// values, and where its count of lookup records is.
#[allow(clippy::type_complexity)]
fn read_chain(
  t: Data<'_>,
  rule: usize,
) -> Option<(Vec<u16>, Vec<u16>, Vec<u16>, usize)> {
  let nb = t.u16(rule)? as usize;
  let back = (0..nb)
    .map(|k| t.u16(rule + 2 + 2 * k))
    .collect::<Option<Vec<_>>>()?;
  let at = rule + 2 + 2 * nb;
  let ni = t.u16(at)? as usize;
  let input = (1..ni)
    .map(|k| t.u16(at + 2 + 2 * (k - 1)))
    .collect::<Option<Vec<_>>>()?;
  let at = at + 2 + 2 * ni.saturating_sub(1);
  let na = t.u16(at)? as usize;
  let ahead = (0..na)
    .map(|k| t.u16(at + 2 + 2 * k))
    .collect::<Option<Vec<_>>>()?;
  Some((back, input, ahead, at + 2 + 2 * na))
}

/// The flag bit that says a lookup is for right-to-left cursive
/// attachment; kept for the follow-up on `curs`.
#[allow(dead_code)]
pub(crate) const CURSIVE_RIGHT_TO_LEFT: u16 = RIGHT_TO_LEFT;
