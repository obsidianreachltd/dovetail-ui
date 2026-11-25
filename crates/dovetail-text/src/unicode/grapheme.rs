//! Extended grapheme clusters (Unicode Standard Annex #29): the units a
//! user sees as one character, such as "é" written as "e" plus an accent,
//! or a family emoji joined with zero-width joiners. The caret moves, and
//! Backspace deletes, by these units.

use super::tables::{
  EXTENDED_PICTOGRAPHIC, GRAPHEME_BREAK, INDIC_CONJUNCT_BREAK,
};

/// Grapheme_Cluster_Break property values.
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gcb {
  Other,
  CR,
  LF,
  Control,
  Extend,
  ZWJ,
  RegionalIndicator,
  Prepend,
  SpacingMark,
  L,
  V,
  T,
  LV,
  LVT,
}

/// Indic_Conjunct_Break property values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InCb {
  None,
  Linker,
  Consonant,
  Extend,
}

fn find<T: Copy>(
  ranges: &[(u32, u32, T)],
  c: char,
) -> Option<T> {
  let c = c as u32;
  let i = ranges.partition_point(|r| r.1 < c);
  ranges.get(i).filter(|r| r.0 <= c).map(|r| r.2)
}

fn gcb(c: char) -> Gcb {
  find(GRAPHEME_BREAK, c).unwrap_or(Gcb::Other)
}

fn incb(c: char) -> InCb {
  find(INDIC_CONJUNCT_BREAK, c).unwrap_or(InCb::None)
}

fn pictographic(c: char) -> bool {
  let c = c as u32;
  let i = EXTENDED_PICTOGRAPHIC.partition_point(|r| r.1 < c);
  EXTENDED_PICTOGRAPHIC
    .get(i)
    .is_some_and(|r| r.0 <= c)
}

/// What the characters so far say about rules that look further back than
/// one character (GB9c, GB11 and GB12/13).
#[derive(Default)]
struct Context {
  /// Regional indicators in a row just before here.
  regional: usize,
  /// After Extended_Pictographic Extend*, then (when `zwj`) a ZWJ.
  pictographic: bool,
  zwj: bool,
  /// Inside Consonant [Extend Linker]*; `linker` once a Linker is seen.
  conjunct: bool,
  linker: bool,
}

/// Whether there is a boundary between `before` and `after`, given what
/// came earlier. Follows the rules of UAX #29 in order.
fn breaks(
  before: char,
  after: char,
  context: &Context,
) -> bool {
  use Gcb::*;
  let (a, b) = (gcb(before), gcb(after));
  match (a, b) {
    (CR, LF) => false,                                       // GB3
    (CR | LF | Control, _) | (_, CR | LF | Control) => true, // GB4, GB5
    (L, L | V | LV | LVT) => false,                          // GB6
    (LV | V, V | T) => false,                                // GB7
    (LVT | T, T) => false,                                   // GB8
    (_, Extend | ZWJ) => false,                              // GB9
    (_, SpacingMark) => false,                               // GB9a
    (Prepend, _) => false,                                   // GB9b
    _ if context.conjunct
      && context.linker
      && incb(after) == InCb::Consonant =>
    {
      false
    }, // GB9c
    (ZWJ, _) if context.zwj && pictographic(after) => false, // GB11
    (RegionalIndicator, RegionalIndicator) => {
      context.regional.is_multiple_of(2)
    }, // GB12, GB13
    _ => true,                                               // GB999
  }
}

/// Updates the context after `c`.
fn advance(
  context: &mut Context,
  c: char,
) {
  let g = gcb(c);
  context.regional = if g == Gcb::RegionalIndicator {
    context.regional + 1
  } else {
    0
  };

  if pictographic(c) {
    context.pictographic = true;
    context.zwj = false;
  } else if g == Gcb::Extend && context.pictographic && !context.zwj {
    // Still Extended_Pictographic Extend*.
  } else if g == Gcb::ZWJ && context.pictographic && !context.zwj {
    context.zwj = true;
  } else {
    context.pictographic = false;
    context.zwj = false;
  }

  match incb(c) {
    InCb::Consonant => {
      context.conjunct = true;
      context.linker = false;
    },
    InCb::Linker if context.conjunct => context.linker = true,
    InCb::Extend if context.conjunct => {},
    _ => {
      context.conjunct = false;
      context.linker = false;
    },
  }
}

/// The byte offsets where grapheme clusters start, and the end of the
/// text: always starts with 0 and ends with `text.len()`.
pub fn grapheme_boundaries(text: &str) -> Vec<usize> {
  let mut out = vec![0];
  let mut context = Context::default();
  let mut previous: Option<char> = None;
  for (i, c) in text.char_indices() {
    if let Some(p) = previous
      && breaks(p, c, &context)
    {
      out.push(i);
    }
    advance(&mut context, c);
    previous = Some(c);
  }
  if !text.is_empty() {
    out.push(text.len());
  }
  out
}

/// The grapheme cluster boundary after `at`, or the end of the text.
pub fn next_grapheme(
  text: &str,
  at: usize,
) -> usize {
  grapheme_boundaries(text)
    .into_iter()
    .find(|&b| b > at)
    .unwrap_or(text.len())
}

/// The grapheme cluster boundary before `at`, or 0.
pub fn previous_grapheme(
  text: &str,
  at: usize,
) -> usize {
  grapheme_boundaries(text)
    .into_iter()
    .rev()
    .find(|&b| b < at)
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn plain_letters_are_one_cluster_each() {
    assert_eq!(grapheme_boundaries("abc"), [0, 1, 2, 3]);
    assert_eq!(grapheme_boundaries(""), [0]);
  }

  #[test]
  fn an_accent_joins_its_letter() {
    let text = "e\u{301}x"; // e + combining acute, then x
    assert_eq!(grapheme_boundaries(text), [0, 3, 4]);
    assert_eq!(next_grapheme(text, 0), 3);
    assert_eq!(previous_grapheme(text, 3), 0);
  }

  #[test]
  fn crlf_is_one_cluster_and_flags_pair_up() {
    assert_eq!(grapheme_boundaries("a\r\nb"), [0, 1, 3, 4]);
    let flags = "\u{1F1EC}\u{1F1E7}\u{1F1EB}\u{1F1F7}"; // GB, FR
    assert_eq!(grapheme_boundaries(flags), [0, 8, 16]);
  }

  #[test]
  fn emoji_joined_with_zwj_are_one_cluster() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    assert_eq!(grapheme_boundaries(family), [0, family.len()]);
  }
}
