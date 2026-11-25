//! The Unicode bidirectional algorithm (UAX #9), in full (M13, D0018):
//! the paragraph level (P2–P3), explicit levels (X1–X10), weak types
//! (W1–W7), bracket pairs (N0), neutrals (N1–N2), implicit levels (I1–I2),
//! and on a line, trailing white space (L1) and visual order (L2).
//!
//! One paragraph is resolved at a time: text without paragraph
//! separators, except perhaps at its end.

use crate::unicode::{BidiClass, bidi_class, bracket};
use BidiClass::*;

/// The deepest embedding level (BD2).
const MAX_DEPTH: u8 = 125;

/// A paragraph's levels: one for each character, in text order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidiLevels {
  /// The paragraph's level: 0 runs left to right, 1 right to left.
  pub paragraph: u8,
  /// Each character's level, after L1. Odd levels run right to left.
  /// Characters that X9 removes (embedding controls and boundary
  /// neutrals) take the level of the character before them.
  pub levels: Vec<u8>,
}

impl BidiLevels {
  /// Resolves the levels of `text`, one paragraph shown on one line.
  /// `paragraph` forces the paragraph's level; `None` takes it from the
  /// first strong letter (P2–P3).
  pub fn new(
    text: &str,
    paragraph: Option<u8>,
  ) -> BidiLevels {
    let chars: Vec<char> = text.chars().collect();
    let classes: Vec<BidiClass> =
      chars.iter().map(|&c| bidi_class(c)).collect();
    let brackets: Vec<Option<(char, bool)>> =
      chars.iter().map(|&c| bracket_key(c)).collect();
    let (paragraph, levels) = resolve(&classes, &brackets, paragraph);
    let mut last = paragraph;
    let levels = levels
      .into_iter()
      .map(|l| {
        let l = l.unwrap_or(last);
        last = l;
        l
      })
      .collect();
    BidiLevels { paragraph, levels }
  }

  /// True when the paragraph runs right to left.
  pub fn is_rtl(&self) -> bool {
    self.paragraph % 2 == 1
  }
}

/// The paragraph's level from its first strong letter, skipping isolates
/// (P2–P3): 1 for R or AL, else 0.
pub fn paragraph_level(text: &str) -> u8 {
  let classes: Vec<BidiClass> = text.chars().map(bidi_class).collect();
  first_strong(&classes).unwrap_or(0)
}

/// A bracket's key and whether it opens: both brackets of a pair have
/// the same key, the opening one's character after canonical mapping.
#[doc(hidden)]
pub fn bracket_key(c: char) -> Option<(char, bool)> {
  bracket(c).map(|(pair, open)| (canonical(if open { c } else { pair }), open))
}

/// Brackets that are the same after canonical decomposition pair up
/// (BD16): the angle brackets U+2329 and U+232A are U+3008 and U+3009.
fn canonical(c: char) -> char {
  match c {
    '\u{2329}' => '\u{3008}',
    '\u{232A}' => '\u{3009}',
    c => c,
  }
}

fn is_isolate_initiator(c: BidiClass) -> bool {
  matches!(c, Lri | Rli | Fsi)
}

fn is_removed(c: BidiClass) -> bool {
  matches!(c, Rle | Lre | Rlo | Lro | Pdf | Bn)
}

/// The level of the first strong letter, skipping isolates and stopping
/// at a paragraph separator.
fn first_strong(classes: &[BidiClass]) -> Option<u8> {
  let mut depth = 0usize;
  for &c in classes {
    match c {
      Lri | Rli | Fsi => depth += 1,
      // An unmatched PDI is skipped like any other character.
      Pdi => depth = depth.saturating_sub(1),
      B => return None,
      L if depth == 0 => return Some(0),
      R | Al if depth == 0 => return Some(1),
      _ => {},
    }
  }
  None
}

/// For each isolate initiator, the index of its matching PDI (BD9).
fn matching_pdis(classes: &[BidiClass]) -> Vec<Option<usize>> {
  let mut matches = vec![None; classes.len()];
  let mut open: Vec<usize> = Vec::new();
  for (i, &c) in classes.iter().enumerate() {
    match c {
      Lri | Rli | Fsi => open.push(i),
      Pdi => {
        if let Some(start) = open.pop() {
          matches[start] = Some(i);
        }
      },
      B => open.clear(),
      _ => {},
    }
  }
  matches
}

#[derive(Clone, Copy)]
struct Entry {
  level: u8,
  over: Option<BidiClass>,
  isolate: bool,
}

fn next_odd(l: u8) -> u8 {
  if l.is_multiple_of(2) { l + 1 } else { l + 2 }
}

fn next_even(l: u8) -> u8 {
  if l.is_multiple_of(2) { l + 2 } else { l + 1 }
}

/// Resolves a paragraph given each character's class and bracket (from
/// `bracket_key`). Returns the paragraph level
/// and each character's level after L1, or `None` for characters X9
/// removes. Used by the tests with Unicode's test data.
#[doc(hidden)]
pub fn resolve(
  original: &[BidiClass],
  brackets: &[Option<(char, bool)>],
  paragraph: Option<u8>,
) -> (u8, Vec<Option<u8>>) {
  let n = original.len();
  let para = paragraph.unwrap_or_else(|| first_strong(original).unwrap_or(0));
  let pdis = matching_pdis(original);
  let mut classes = original.to_vec();
  let mut levels = vec![para; n];

  // X1–X8: explicit levels.
  let mut stack = vec![Entry { level: para, over: None, isolate: false }];
  let (mut overflow_isolates, mut overflow_embeddings, mut valid_isolates) =
    (0usize, 0usize, 0usize);
  for i in 0..n {
    let top = *stack.last().expect("never empty");
    match original[i] {
      Rle | Lre | Rlo | Lro => {
        levels[i] = top.level;
        let rtl = matches!(original[i], Rle | Rlo);
        let level = if rtl {
          next_odd(top.level)
        } else {
          next_even(top.level)
        };
        if level <= MAX_DEPTH
          && overflow_isolates == 0
          && overflow_embeddings == 0
        {
          let over = match original[i] {
            Rlo => Some(R),
            Lro => Some(L),
            _ => None,
          };
          stack.push(Entry { level, over, isolate: false });
        } else if overflow_isolates == 0 {
          overflow_embeddings += 1;
        }
      },
      Rli | Lri | Fsi => {
        levels[i] = top.level;
        if let Some(o) = top.over {
          classes[i] = o;
        }
        let rtl = match original[i] {
          Rli => true,
          Lri => false,
          _ => {
            let end = pdis[i].unwrap_or(n);
            first_strong(&original[i + 1..end]) == Some(1)
          },
        };
        let level = if rtl {
          next_odd(top.level)
        } else {
          next_even(top.level)
        };
        if level <= MAX_DEPTH
          && overflow_isolates == 0
          && overflow_embeddings == 0
        {
          valid_isolates += 1;
          stack.push(Entry { level, over: None, isolate: true });
        } else {
          overflow_isolates += 1;
        }
      },
      Pdi => {
        if overflow_isolates > 0 {
          overflow_isolates -= 1;
        } else if valid_isolates > 0 {
          overflow_embeddings = 0;
          while stack.last().is_some_and(|e| !e.isolate) {
            stack.pop();
          }
          stack.pop();
          valid_isolates -= 1;
        }
        let top = *stack.last().expect("never empty");
        levels[i] = top.level;
        if let Some(o) = top.over {
          classes[i] = o;
        }
      },
      Pdf => {
        levels[i] = top.level;
        if overflow_isolates > 0 {
        } else if overflow_embeddings > 0 {
          overflow_embeddings -= 1;
        } else if !top.isolate && stack.len() >= 2 {
          stack.pop();
        }
      },
      B => levels[i] = para,
      Bn => levels[i] = top.level,
      _ => {
        levels[i] = top.level;
        if let Some(o) = top.over {
          classes[i] = o;
        }
      },
    }
  }

  // X9–X10: the isolating run sequences of the characters that remain.
  let kept: Vec<usize> = (0..n)
    .filter(|&i| !is_removed(original[i]))
    .collect();
  let mut runs: Vec<Vec<usize>> = Vec::new();
  for &i in &kept {
    match runs.last_mut() {
      Some(run) if levels[*run.last().unwrap()] == levels[i] => run.push(i),
      _ => runs.push(vec![i]),
    }
  }
  let run_of = {
    let mut of = vec![usize::MAX; n];
    for (r, run) in runs.iter().enumerate() {
      for &i in run {
        of[i] = r;
      }
    }
    of
  };
  let matched_pdi: Vec<bool> = {
    let mut m = vec![false; n];
    for p in pdis.iter().flatten() {
      m[*p] = true;
    }
    m
  };
  let mut sequences: Vec<Vec<usize>> = Vec::new();
  for run in &runs {
    let first = run[0];
    if original[first] == Pdi && matched_pdi[first] {
      continue;
    }
    let mut seq = run.clone();
    loop {
      let last = *seq.last().unwrap();
      let next = if is_isolate_initiator(original[last]) {
        pdis[last]
      } else {
        None
      };
      match next {
        Some(p) if run_of[p] != usize::MAX => seq.extend(&runs[run_of[p]]),
        _ => break,
      }
    }
    sequences.push(seq);
  }

  for seq in &sequences {
    resolve_sequence(
      seq,
      original,
      &mut classes,
      &levels,
      brackets,
      para,
      &kept,
    );
  }

  // I1–I2.
  let mut out: Vec<Option<u8>> = vec![None; n];
  for &i in &kept {
    let l = levels[i];
    out[i] = Some(match (l % 2, classes[i]) {
      (0, R) => l + 1,
      (0, An | En) => l + 2,
      (1, L | En | An) => l + 1,
      _ => l,
    });
  }

  // L1: separators, and white space before them and at the end, go to the
  // paragraph level. Removed characters there do too.
  let mut trailing = true;
  for i in (0..n).rev() {
    match original[i] {
      S | B => {
        out[i] = Some(para);
        trailing = true;
      },
      Ws | Fsi | Lri | Rli | Pdi if trailing => out[i] = Some(para),
      c if is_removed(c) => {},
      _ => trailing = false,
    }
  }
  (para, out)
}

fn strong_of(c: BidiClass) -> Option<BidiClass> {
  match c {
    L => Some(L),
    R | Al | En | An => Some(R),
    _ => None,
  }
}

fn is_neutral(c: BidiClass) -> bool {
  matches!(c, B | S | Ws | On | Fsi | Lri | Rli | Pdi)
}

#[allow(clippy::too_many_arguments)]
fn resolve_sequence(
  seq: &[usize],
  original: &[BidiClass],
  classes: &mut [BidiClass],
  levels: &[u8],
  brackets: &[Option<(char, bool)>],
  para: u8,
  kept: &[usize],
) {
  let level = levels[seq[0]];
  let embedding = if level.is_multiple_of(2) { L } else { R };
  // sos and eos (X10): the higher of this level and the neighbours'.
  let position = |i: usize| kept.partition_point(|&k| k < i);
  let before = position(seq[0])
    .checked_sub(1)
    .map_or(para, |p| levels[kept[p]]);
  let last = *seq.last().unwrap();
  let after = if is_isolate_initiator(original[last]) {
    para
  } else {
    kept
      .get(position(last) + 1)
      .map_or(para, |&k| levels[k])
  };
  let dir = |l: u8| if l.is_multiple_of(2) { L } else { R };
  let sos = dir(level.max(before));
  let eos = dir(level.max(after));

  let mut t: Vec<BidiClass> = seq.iter().map(|&i| classes[i]).collect();
  let len = t.len();

  // W1.
  for k in 0..len {
    if t[k] == Nsm {
      t[k] = if k == 0 {
        sos
      } else if matches!(t[k - 1], Lri | Rli | Fsi | Pdi) {
        On
      } else {
        t[k - 1]
      };
    }
  }
  // W2, W3.
  let mut last_strong = sos;
  for c in t.iter_mut() {
    match *c {
      L | R | Al => last_strong = *c,
      En if last_strong == Al => *c = An,
      _ => {},
    }
  }
  for c in t.iter_mut() {
    if *c == Al {
      *c = R;
    }
  }
  // W4.
  for k in 1..len.saturating_sub(1) {
    match (t[k - 1], t[k], t[k + 1]) {
      (En, Es | Cs, En) => t[k] = En,
      (An, Cs, An) => t[k] = An,
      _ => {},
    }
  }
  // W5.
  let mut k = 0;
  while k < len {
    if t[k] == Et {
      let start = k;
      while k < len && t[k] == Et {
        k += 1;
      }
      let touches =
        (start > 0 && t[start - 1] == En) || (k < len && t[k] == En);
      if touches {
        t[start..k].fill(En);
      }
    } else {
      k += 1;
    }
  }
  // W6.
  for c in t.iter_mut() {
    if matches!(*c, Es | Et | Cs) {
      *c = On;
    }
  }
  // W7.
  let mut last_strong = sos;
  for c in t.iter_mut() {
    match *c {
      L | R => last_strong = *c,
      En if last_strong == L => *c = L,
      _ => {},
    }
  }

  // N0: bracket pairs.
  let mut pairs: Vec<(usize, usize)> = Vec::new();
  let mut open: Vec<(char, usize)> = Vec::new();
  for k in 0..len {
    let i = seq[k];
    let Some((key, opens)) = brackets[i] else {
      continue;
    };
    if t[k] != On {
      continue;
    }
    if opens {
      if open.len() == 63 {
        break;
      }
      open.push((key, k));
    } else {
      if let Some(found) = open.iter().rposition(|&(p, _)| p == key) {
        pairs.push((open[found].1, k));
        open.truncate(found);
      }
    }
  }
  pairs.sort_unstable();
  for (a, b) in pairs {
    let mut inside_e = false;
    let mut inside_other = false;
    for c in &t[a + 1..b] {
      match strong_of(*c) {
        Some(s) if s == embedding => inside_e = true,
        Some(_) => inside_other = true,
        None => {},
      }
    }
    let set = if inside_e {
      Some(embedding)
    } else if inside_other {
      let context = t[..a]
        .iter()
        .rev()
        .find_map(|c| strong_of(*c))
        .unwrap_or(sos);
      Some(if context != embedding {
        context
      } else {
        embedding
      })
    } else {
      None
    };
    if let Some(s) = set {
      for k in [a, b] {
        t[k] = s;
        // Marks after a bracket follow it.
        let mut m = k + 1;
        while m < len && original[seq[m]] == Nsm {
          t[m] = s;
          m += 1;
        }
      }
    }
  }

  // N1–N2.
  let mut k = 0;
  while k < len {
    if !is_neutral(t[k]) {
      k += 1;
      continue;
    }
    let start = k;
    while k < len && is_neutral(t[k]) {
      k += 1;
    }
    let prev = if start == 0 {
      sos
    } else {
      strong_of(t[start - 1]).unwrap_or(embedding)
    };
    let next = if k == len {
      eos
    } else {
      strong_of(t[k]).unwrap_or(embedding)
    };
    let fill = if prev == next { prev } else { embedding };
    t[start..k].fill(fill);
  }

  for (k, &i) in seq.iter().enumerate() {
    classes[i] = t[k];
  }
}

/// The visual order of a line: the index of the character shown first,
/// then the next, left to right (L2). Every level from the highest down
/// to the lowest odd one reverses the runs at or above it.
pub fn visual_order(levels: &[u8]) -> Vec<usize> {
  let mut order: Vec<usize> = (0..levels.len()).collect();
  let Some(&highest) = levels.iter().max() else {
    return order;
  };
  let lowest_odd = levels
    .iter()
    .copied()
    .filter(|l| l % 2 == 1)
    .min()
    .unwrap_or(highest + 1);
  let mut level = highest;
  while level >= lowest_odd && level > 0 {
    let mut k = 0;
    while k < order.len() {
      if levels[order[k]] >= level {
        let start = k;
        while k < order.len() && levels[order[k]] >= level {
          k += 1;
        }
        order[start..k].reverse();
      } else {
        k += 1;
      }
    }
    level -= 1;
  }
  order
}

#[cfg(test)]
mod tests {
  use super::*;

  fn levels(text: &str) -> Vec<u8> {
    BidiLevels::new(text, None).levels
  }

  #[test]
  fn plain_text_keeps_its_order() {
    assert_eq!(levels("abc"), [0, 0, 0]);
    assert_eq!(visual_order(&levels("abc")), [0, 1, 2]);
    assert_eq!(paragraph_level("abc"), 0);
  }

  #[test]
  fn arabic_in_english_is_reversed() {
    // "ab ARB cd" with Arabic letters: the Arabic run is level 1.
    let text = "ab \u{0627}\u{0628}\u{062A} cd";
    let l = levels(text);
    assert_eq!(l, [0, 0, 0, 1, 1, 1, 0, 0, 0]);
    assert_eq!(visual_order(&l), [0, 1, 2, 5, 4, 3, 6, 7, 8]);
  }

  #[test]
  fn an_arabic_paragraph_runs_right_to_left_with_numbers_left_to_right() {
    let text = "\u{0627}\u{0628} 12";
    let b = BidiLevels::new(text, None);
    assert!(b.is_rtl());
    assert_eq!(b.levels, [1, 1, 1, 2, 2]);
    assert_eq!(visual_order(&b.levels), [3, 4, 2, 1, 0]);
  }

  #[test]
  fn brackets_take_the_direction_inside_them() {
    // Hebrew with English in brackets, in a right-to-left paragraph.
    let text = "\u{05D0}(b)";
    let b = BidiLevels::new(text, None);
    assert_eq!(b.levels, [1, 1, 2, 1]);
  }
}
