//! Shaping one line of text (M06, M13): bidirectional levels, runs by
//! font, script and level, Arabic joining forms and `GSUB`, kerning, mark
//! positioning from `GPOS`, and the visual order. Also where carets go,
//! what a selection covers, and moving the caret in visual order.

use std::rc::Rc;

use crate::bidi::{BidiLevels, visual_order};
use crate::font::{Font, LineMetrics};
use crate::otl::{Attachment, Glyph, Layout, glyph_class};
use crate::unicode::{
  BidiClass, JoiningType, bidi_class, joining_type, mirror,
};

/// One glyph placed on a line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapedGlyph {
  /// Which font of the [`Fonts`](crate::Fonts) it comes from.
  pub font: usize,
  /// The glyph in that font.
  pub glyph: u16,
  /// The byte offset in the text of the first character it shows.
  pub byte: usize,
  /// One past the last byte of the characters it shows (more than one
  /// for a ligature).
  pub byte_end: usize,
  /// The pen position, in pixels from the line's left end.
  pub x: f32,
  /// How far the pen moves after it, in pixels. Marks have none.
  pub advance: f32,
  /// How far above the baseline it goes, in pixels: marks are raised or
  /// lowered onto their letter.
  pub y: f32,
  /// It belongs to text that runs right to left.
  pub rtl: bool,
}

/// Where one grapheme cluster (a user-perceived character) is on a line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClusterPlace {
  /// Its first byte in the text.
  pub start: usize,
  /// One past its last byte.
  pub end: usize,
  /// Its left edge, in pixels from the line's left end.
  pub left: f32,
  /// Its right edge.
  pub right: f32,
  /// It runs right to left.
  pub rtl: bool,
}

/// One line of text, shaped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapedLine {
  /// The glyphs, from left to right as shown.
  pub glyphs: Vec<ShapedGlyph>,
  /// The width of the line in pixels.
  pub width: f32,
  /// The main font's line metrics at the size shaped.
  pub metrics: LineMetrics,
  /// The line is a paragraph that runs right to left: it lines up on the
  /// right.
  pub rtl: bool,
  /// Where each grapheme cluster is, in text order.
  pub clusters: Vec<ClusterPlace>,
  /// The length of the text, in bytes.
  pub len: usize,
}

impl ShapedLine {
  /// Where the caret goes before byte `at`, in pixels from the line's left
  /// end. There is one caret: next to the cluster before it in the text,
  /// on that cluster's trailing side (D0018). At the start, it is at the
  /// paragraph's start: the left end, or the right end for right to left.
  pub fn caret_x(
    &self,
    at: usize,
  ) -> f32 {
    match self.clusters.iter().rev().find(|c| c.start < at) {
      Some(c) if c.rtl => c.left,
      Some(c) => c.right,
      None if self.rtl => self.width,
      None => 0.0,
    }
  }

  /// The caret places: every cluster boundary, with its x.
  pub fn caret_stops(&self) -> Vec<(usize, f32)> {
    let mut stops: Vec<usize> = self.clusters.iter().map(|c| c.start).collect();
    stops.push(self.len);
    stops.dedup();
    stops
      .into_iter()
      .map(|b| (b, self.caret_x(b)))
      .collect()
  }

  /// The caret place nearest to `x`: where a click there puts the caret.
  pub fn byte_at(
    &self,
    x: f32,
  ) -> usize {
    self
      .caret_stops()
      .into_iter()
      .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
      .map_or(0, |s| s.0)
  }

  /// The pieces of the line that bytes `from..to` cover, as left and right
  /// edges from left to right: one piece for text in one direction, more
  /// when it mixes directions.
  pub fn selection(
    &self,
    from: usize,
    to: usize,
  ) -> Vec<(f32, f32)> {
    let mut pieces: Vec<(f32, f32)> = self
      .clusters
      .iter()
      .filter(|c| c.start >= from && c.end <= to && c.right > c.left)
      .map(|c| (c.left, c.right))
      .collect();
    pieces.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for p in pieces {
      match merged.last_mut() {
        Some(last) if p.0 <= last.1 + 0.01 => last.1 = last.1.max(p.1),
        _ => merged.push(p),
      }
    }
    merged
  }

  /// The caret place one step to the left (or right) of `at` on the
  /// screen, whichever way the text runs. `None` at the end of the line.
  pub fn move_visually(
    &self,
    at: usize,
    right: bool,
  ) -> Option<usize> {
    let mut stops = self.caret_stops();
    // Left to right on the screen; places at the same x in text order.
    stops.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let i = stops.iter().position(|s| s.0 == at)?;
    let next = if right {
      i.checked_add(1)?
    } else {
      i.checked_sub(1)?
    };
    stops.get(next).map(|s| s.0)
  }
}

/// What shaping needs from the fonts.
pub(crate) trait FontSource {
  /// The font and glyph for a character, after fallback.
  fn choose(
    &mut self,
    c: char,
  ) -> Option<(usize, u16)>;
  /// Font `i`.
  fn font(
    &mut self,
    i: usize,
  ) -> Option<Rc<Font>>;
  /// Kerning between two glyphs of font `i`, in font units.
  fn kerning(
    &mut self,
    i: usize,
    left: u16,
    right: u16,
  ) -> f32;
}

/// Arabic and the scripts that join like it are shaped with `GSUB`.
fn is_arabic(c: char) -> bool {
  matches!(c as u32,
    0x0600..=0x06FF | 0x0750..=0x077F | 0x0870..=0x089F | 0x08A0..=0x08FF
    | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF | 0x10EC0..=0x10EFF | 0x1EE00..=0x1EEFF)
}

/// Characters that take the script and font of the character before
/// them: marks and joiners.
fn inherits(c: char) -> bool {
  bidi_class(c) == BidiClass::Nsm || matches!(c, '\u{200C}' | '\u{200D}')
}

/// Characters that show nothing: controls and direction marks.
fn invisible(c: char) -> bool {
  c.is_control()
    || matches!(
      bidi_class(c),
      BidiClass::Bn
        | BidiClass::Lre
        | BidiClass::Rle
        | BidiClass::Lro
        | BidiClass::Rlo
        | BidiClass::Pdf
        | BidiClass::Lri
        | BidiClass::Rli
        | BidiClass::Fsi
        | BidiClass::Pdi
    )
    || matches!(c, '\u{200E}' | '\u{200F}' | '\u{061C}')
}

const CCMP: u32 = 1 << 0;
const ISOL: u32 = 1 << 1;
const FINA: u32 = 1 << 2;
const MEDI: u32 = 1 << 3;
const INIT: u32 = 1 << 4;
const RLIG: u32 = 1 << 5;
const CALT: u32 = 1 << 6;
const LIGA: u32 = 1 << 7;
const MARK: u32 = 1 << 0;
const MKMK: u32 = 1 << 1;

/// The joining form of each character (as a feature mask bit), from the
/// Joining_Type of it and its neighbours, skipping transparent ones.
fn joining_forms(chars: &[char]) -> Vec<u32> {
  let types: Vec<JoiningType> = chars
    .iter()
    .map(|&c| {
      if is_arabic(c) || matches!(c, '\u{200D}') {
        joining_type(c)
      } else if joining_type(c) == JoiningType::T {
        JoiningType::T
      } else {
        JoiningType::U
      }
    })
    .collect();
  let mut forms = vec![ISOL; chars.len()];
  let mut joins_next = vec![false; chars.len()];
  let mut joins_prev = vec![false; chars.len()];
  let mut prev: Option<usize> = None;
  for i in 0..chars.len() {
    let t = types[i];
    if t == JoiningType::T {
      continue;
    }
    if let Some(p) = prev
      && matches!(types[p], JoiningType::D | JoiningType::L | JoiningType::C)
      && matches!(t, JoiningType::D | JoiningType::R | JoiningType::C)
    {
      joins_next[p] = true;
      joins_prev[i] = true;
    }
    prev = Some(i);
  }
  for i in 0..chars.len() {
    forms[i] = match (joins_prev[i], joins_next[i]) {
      (true, true) => MEDI,
      (false, true) => INIT,
      (true, false) => FINA,
      (false, false) => ISOL,
    };
  }
  forms
}

/// One glyph of a run, before it is placed.
struct Piece {
  font: usize,
  glyph: u16,
  /// Characters, as indexes into the line's characters.
  first: usize,
  end: usize,
  /// In font units.
  advance: f32,
  /// For a mark: where it goes, in font units, on a glyph of this run.
  attach: Option<Attachment>,
  scale: f32,
}

/// Shapes one line of `text` at `size` pixels.
pub(crate) fn shape(
  source: &mut dyn FontSource,
  text: &str,
  size: f32,
  metrics: LineMetrics,
) -> ShapedLine {
  let chars: Vec<(usize, char)> = text.char_indices().collect();
  let n = chars.len();
  let bidi = BidiLevels::new(text, None);
  let levels = &bidi.levels;
  let only: Vec<char> = chars.iter().map(|c| c.1).collect();
  let forms = joining_forms(&only);

  // The font and script of each character; marks follow their letter.
  let mut font_of: Vec<Option<usize>> = vec![None; n];
  let mut arabic = vec![false; n];
  for i in 0..n {
    let c = only[i];
    if invisible(c) {
      continue;
    }
    if inherits(c) && i > 0 {
      arabic[i] = arabic[i - 1];
      if let Some(f) = font_of[i - 1]
        && source
          .font(f)
          .and_then(|f| f.glyph_index(c))
          .is_some()
      {
        font_of[i] = Some(f);
        continue;
      }
    } else {
      arabic[i] = is_arabic(c);
    }
    font_of[i] = source.choose(c).map(|(f, _)| f);
  }

  // Runs of characters with the same level, font and script.
  let mut runs: Vec<(usize, usize)> = Vec::new();
  let mut start = 0;
  for i in 1..=n {
    let split = i == n
      || levels[i] != levels[start]
      || arabic[i] != arabic[start]
      || (font_of[i].is_some()
        && font_of[start].is_some()
        && font_of[i] != font_of[start]);
    if split {
      if start < i {
        runs.push((start, i));
      }
      start = i;
    }
  }

  let mut shaped_runs: Vec<(usize, Vec<Piece>)> = Vec::new();
  for &(a, b) in &runs {
    let rtl = levels[a] % 2 == 1;
    let pieces = if arabic[a] {
      shape_arabic(source, &only, &font_of, &forms, a, b, size, rtl)
    } else {
      shape_simple(source, &only, &font_of, a, b, size, rtl)
    };
    shaped_runs.push((a, pieces));
  }

  // Runs in visual order: by where their first character goes.
  let order = visual_order(levels);
  let mut position = vec![0; n];
  for (v, &i) in order.iter().enumerate() {
    position[i] = v;
  }
  let mut visual: Vec<usize> = (0..shaped_runs.len()).collect();
  visual.sort_by_key(|&r| {
    let (a, b) = runs[r];
    (a..b).map(|i| position[i]).min().unwrap_or(0)
  });

  let mut glyphs: Vec<ShapedGlyph> = Vec::new();
  let mut x = 0.0;
  for r in visual {
    let (a, pieces) = &shaped_runs[r];
    let rtl = levels[*a] % 2 == 1;
    let first_glyph = glyphs.len();
    let mut placed: Vec<Option<usize>> = vec![None; pieces.len()];
    let order: Vec<usize> = if rtl {
      (0..pieces.len()).rev().collect()
    } else {
      (0..pieces.len()).collect()
    };
    for k in order {
      let p = &pieces[k];
      let advance = p.advance * p.scale;
      placed[k] = Some(glyphs.len());
      glyphs.push(ShapedGlyph {
        font: p.font,
        glyph: p.glyph,
        byte: chars[p.first].0,
        byte_end: chars.get(p.end).map_or(text.len(), |c| c.0),
        x,
        advance,
        y: 0.0,
        rtl,
      });
      if p.attach.is_none() {
        x += advance;
      }
    }
    // Marks go on their letter, wherever it was placed.
    for (k, p) in pieces.iter().enumerate() {
      if let (Some(a), Some(me)) = (p.attach, placed[k])
        && let Some(base) = placed.get(a.base).copied().flatten()
      {
        glyphs[me].x = glyphs[base].x + a.dx * p.scale;
        glyphs[me].y = glyphs[base].y + a.dy * p.scale;
      }
    }
    let _ = first_glyph;
  }

  let clusters =
    place_clusters(text, &chars, levels, &glyphs, bidi.is_rtl(), x);
  ShapedLine {
    glyphs,
    width: x,
    metrics,
    rtl: bidi.is_rtl(),
    clusters,
    len: text.len(),
  }
}

/// Text that needs no `GSUB`: one glyph per character, with kerning, and
/// mirrored brackets when it runs right to left.
fn shape_simple(
  source: &mut dyn FontSource,
  chars: &[char],
  font_of: &[Option<usize>],
  a: usize,
  b: usize,
  size: f32,
  rtl: bool,
) -> Vec<Piece> {
  let mut pieces: Vec<Piece> = Vec::new();
  for i in a..b {
    let c = chars[i];
    let Some(font) = font_of[i] else {
      continue;
    };
    let Some(f) = source.font(font) else {
      continue;
    };
    let shown = if rtl {
      mirror(c)
        .filter(|m| f.glyph_index(*m).is_some())
        .unwrap_or(c)
    } else {
      c
    };
    let glyph = f
      .glyph_index(shown)
      .or_else(|| source.choose(c).map(|g| g.1))
      .unwrap_or(0);
    let scale = size / f.units_per_em();
    let mut advance = f.advance(glyph);
    if let Some(prev) = pieces.last_mut()
      && prev.font == font
    {
      let kern = source.kerning(font, prev.glyph, glyph);
      // The space between two letters belongs to the one on the left.
      if rtl {
        advance += kern;
      } else {
        prev.advance += kern;
      }
    }
    pieces.push(Piece {
      font,
      glyph,
      first: i,
      end: i + 1,
      advance,
      attach: None,
      scale,
    });
  }
  pieces
}

/// Arabic: joining forms and `GSUB`, then kerning and marks.
#[allow(clippy::too_many_arguments)]
fn shape_arabic(
  source: &mut dyn FontSource,
  chars: &[char],
  font_of: &[Option<usize>],
  forms: &[u32],
  a: usize,
  b: usize,
  size: f32,
  rtl: bool,
) -> Vec<Piece> {
  let Some(font) = (a..b).find_map(|i| font_of[i]) else {
    return Vec::new();
  };
  let Some(f) = source.font(font) else {
    return Vec::new();
  };
  let common = CCMP | RLIG | CALT | LIGA;
  let mut buffer: Vec<Glyph> = (a..b)
    .filter(|&i| !invisible(chars[i]))
    .map(|i| {
      let c = chars[i];
      let shown = if rtl {
        mirror(c)
          .filter(|m| f.glyph_index(*m).is_some())
          .unwrap_or(c)
      } else {
        c
      };
      Glyph {
        glyph: f.glyph_index(shown).unwrap_or(0),
        first: i,
        end: i + 1,
        mask: common | forms[i],
        ligature: 0,
        component: 0,
      }
    })
    .collect();
  let features = [
    (*b"ccmp", CCMP),
    (*b"isol", ISOL),
    (*b"fina", FINA),
    (*b"medi", MEDI),
    (*b"init", INIT),
    (*b"rlig", RLIG),
    (*b"calt", CALT),
    (*b"liga", LIGA),
  ];
  if let Some(gsub) = Layout::new(&f, true, b"arab", &features) {
    gsub.substitute(&mut buffer);
  }
  let gdef = f.table(b"GDEF");
  let scale = size / f.units_per_em();
  let mut pieces: Vec<Piece> = buffer
    .iter()
    .map(|g| {
      let mark = glyph_class(gdef, g.glyph) == 3;
      Piece {
        font,
        glyph: g.glyph,
        first: g.first,
        end: g.end,
        advance: if mark { 0.0 } else { f.advance(g.glyph) },
        attach: None,
        scale,
      }
    })
    .collect();
  // Kerning between letters, skipping marks.
  let mut prev: Option<usize> = None;
  for k in 0..pieces.len() {
    if glyph_class(gdef, pieces[k].glyph) == 3 {
      continue;
    }
    if let Some(p) = prev {
      let kern = source.kerning(font, pieces[p].glyph, pieces[k].glyph);
      if rtl {
        pieces[k].advance += kern;
      } else {
        pieces[p].advance += kern;
      }
    }
    prev = Some(k);
  }
  let marks = [(*b"mark", MARK), (*b"mkmk", MKMK)];
  for g in &mut buffer {
    g.mask = MARK | MKMK;
  }
  if let Some(gpos) = Layout::new(&f, false, b"arab", &marks) {
    for (k, a) in gpos
      .position_marks(&buffer)
      .into_iter()
      .enumerate()
    {
      pieces[k].attach = a;
    }
  }
  // A mark with nothing to go on stays where the pen is, with no advance.
  pieces
}

/// Where each grapheme cluster is, from the glyphs that show it. A
/// ligature's width is shared by the clusters it shows.
fn place_clusters(
  text: &str,
  chars: &[(usize, char)],
  levels: &[u8],
  glyphs: &[ShapedGlyph],
  rtl: bool,
  width: f32,
) -> Vec<ClusterPlace> {
  let bounds = crate::grapheme_boundaries(text);
  let mut clusters: Vec<ClusterPlace> = bounds
    .windows(2)
    .map(|w| {
      let level = chars
        .iter()
        .position(|c| c.0 == w[0])
        .map_or(0, |i| levels[i]);
      ClusterPlace {
        start: w[0],
        end: w[1],
        left: f32::INFINITY,
        right: f32::NEG_INFINITY,
        rtl: level % 2 == 1,
      }
    })
    .collect();
  for g in glyphs.iter().filter(|g| g.advance > 0.0) {
    let covered: Vec<usize> = clusters
      .iter()
      .enumerate()
      .filter(|(_, c)| c.start >= g.byte && c.start < g.byte_end)
      .map(|(k, _)| k)
      .collect();
    let covered = if covered.is_empty() {
      // Inside a cluster that starts earlier.
      clusters
        .iter()
        .rposition(|c| c.start <= g.byte)
        .into_iter()
        .collect()
    } else {
      covered
    };
    let parts = covered.len().max(1) as f32;
    let step = g.advance / parts;
    for (k, &c) in covered.iter().enumerate() {
      let k = if g.rtl { covered.len() - 1 - k } else { k } as f32;
      let left = g.x + step * k;
      let place = &mut clusters[c];
      place.left = place.left.min(left);
      place.right = place.right.max(left + step);
    }
  }
  // Clusters that show nothing sit at the caret before them.
  let mut caret = if rtl { width } else { 0.0 };
  for c in &mut clusters {
    if c.left > c.right {
      c.left = caret;
      c.right = caret;
    }
    caret = if c.rtl { c.left } else { c.right };
  }
  clusters
}
