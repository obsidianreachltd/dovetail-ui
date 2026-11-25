//! Character properties for bidirectional text and Arabic joining (M13),
//! from the generated tables.

use super::bidi_tables::{BIDI_CLASS, BRACKETS, JOINING_TYPE, MIRRORS};

/// A character's Bidi_Class, which says how it behaves in bidirectional
/// text (UAX #9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BidiClass {
  /// Left to right, as Latin letters.
  L,
  /// Right to left, as Hebrew letters.
  R,
  /// Arabic letters.
  Al,
  /// European digits.
  En,
  /// Plus and minus signs.
  Es,
  /// Currency and percent signs, which go with numbers.
  Et,
  /// Arabic digits.
  An,
  /// Separators inside numbers, such as `,` `.` `:` `/`.
  Cs,
  /// Marks that join the character before them.
  Nsm,
  /// Characters with no direction that layout ignores, such as joiners.
  Bn,
  /// A paragraph separator.
  B,
  /// A segment separator, such as Tab.
  S,
  /// White space.
  Ws,
  /// Other neutral characters, such as most punctuation.
  On,
  /// Left-to-right embedding.
  Lre,
  /// Left-to-right override.
  Lro,
  /// Right-to-left embedding.
  Rle,
  /// Right-to-left override.
  Rlo,
  /// Ends an embedding or override.
  Pdf,
  /// Left-to-right isolate.
  Lri,
  /// Right-to-left isolate.
  Rli,
  /// First-strong isolate.
  Fsi,
  /// Ends an isolate.
  Pdi,
}

/// How a character joins its neighbours in cursive scripts such as
/// Arabic (Joining_Type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JoiningType {
  /// Does not join.
  U,
  /// Joins on both sides, but has no forms of its own (tatweel, ZWJ).
  C,
  /// Joins on both sides.
  D,
  /// Joins only the character before it in the text (to its right).
  R,
  /// Joins only the character after it in the text.
  L,
  /// Skipped when joining, as marks are.
  T,
}

fn find<T: Copy>(
  ranges: &[(u32, u32, T)],
  c: char,
) -> Option<T> {
  let c = c as u32;
  let i = ranges.partition_point(|r| r.1 < c);
  ranges.get(i).filter(|r| r.0 <= c).map(|r| r.2)
}

/// The character's Bidi_Class.
pub fn bidi_class(c: char) -> BidiClass {
  find(BIDI_CLASS, c).unwrap_or(BidiClass::L)
}

/// The character's Joining_Type.
pub fn joining_type(c: char) -> JoiningType {
  find(JOINING_TYPE, c).unwrap_or(JoiningType::U)
}

/// For a bracket: its pair, and whether it opens.
pub fn bracket(c: char) -> Option<(char, bool)> {
  let i = BRACKETS
    .binary_search_by_key(&(c as u32), |b| b.0)
    .ok()?;
  let (_, pair, open) = BRACKETS[i];
  Some((char::from_u32(pair)?, open))
}

/// The character's mirror image, for right-to-left text, as `(` for `)`.
pub fn mirror(c: char) -> Option<char> {
  let i = MIRRORS
    .binary_search_by_key(&(c as u32), |m| m.0)
    .ok()?;
  char::from_u32(MIRRORS[i].1)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn properties_of_well_known_characters() {
    assert_eq!(bidi_class('a'), BidiClass::L);
    assert_eq!(bidi_class('\u{05D0}'), BidiClass::R);
    assert_eq!(bidi_class('\u{0628}'), BidiClass::Al);
    assert_eq!(bidi_class('1'), BidiClass::En);
    assert_eq!(bidi_class('\u{0661}'), BidiClass::An);
    assert_eq!(bidi_class(' '), BidiClass::Ws);
    assert_eq!(bidi_class('\u{2067}'), BidiClass::Rli);
    assert_eq!(bidi_class('\u{064E}'), BidiClass::Nsm);
    // Unassigned in an Arabic block takes the block's default.
    assert_ne!(bidi_class('\u{08FF}'), BidiClass::L);
    assert_eq!(joining_type('\u{0628}'), JoiningType::D, "beh");
    assert_eq!(joining_type('\u{0627}'), JoiningType::R, "alef");
    assert_eq!(joining_type('\u{064E}'), JoiningType::T, "fatha");
    assert_eq!(joining_type('\u{0640}'), JoiningType::C, "tatweel");
    assert_eq!(joining_type('a'), JoiningType::U);
    assert_eq!(bracket('('), Some((')', true)));
    assert_eq!(bracket(']'), Some(('[', false)));
    assert_eq!(bracket('a'), None);
    assert_eq!(mirror('('), Some(')'));
    assert_eq!(mirror('<'), Some('>'));
    assert_eq!(mirror('a'), None);
  }
}
