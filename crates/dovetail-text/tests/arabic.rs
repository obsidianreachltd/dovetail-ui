//! Arabic joining, the lam-alef ligature, marks, and mixed-direction lines
//! (M13), with Segoe UI. The tests are skipped where Segoe UI is missing.
//!
//! The expected glyphs are known without Dovetail's shaping:
//!
//! - letter forms come from the font's own Arabic presentation forms
//!   (U+FB50 to U+FEFF), which it maps to the same glyphs its `GSUB` gives;
//! - whole words were shaped by Windows' own shaper (Uniscribe's
//!   `ScriptShape`) on 2026-11-23, with Segoe UI revision 5.72 (5,395
//!   glyphs). Segoe UI draws lam-alef as two joined glyphs, not as its
//!   single presentation-form glyph. Other revisions skip that check.

use std::path::PathBuf;

use dovetail_text::{Font, Fonts};

const SEGOE: &str = "C:/Windows/Fonts/segoeui.ttf";

fn fonts() -> Option<(Fonts, Font)> {
  let path = PathBuf::from(SEGOE);
  if !path.is_file() {
    println!("skipped: {SEGOE} is missing");
    return None;
  }
  let font = Font::from_data(std::fs::read(&path).ok()?, 0).ok()?;
  Some((Fonts::from_files([(path, 0)]), font))
}

/// The glyphs of a line, left to right as shown, without marks.
fn shown(
  fonts: &Fonts,
  text: &str,
) -> Vec<u16> {
  fonts
    .shape(text, 20.0)
    .glyphs
    .iter()
    .filter(|g| g.advance > 0.0)
    .map(|g| g.glyph)
    .collect()
}

/// The glyph a presentation form maps to.
fn form(
  font: &Font,
  c: u32,
) -> u16 {
  font
    .glyph_index(char::from_u32(c).unwrap())
    .unwrap_or_else(|| panic!("the font maps U+{c:04X}"))
}

const BEH: char = '\u{0628}';
const LAM: char = '\u{0644}';
const ALEF: char = '\u{0627}';
const SEEN: char = '\u{0633}';
const MEEM: char = '\u{0645}';
const FATHA: char = '\u{064E}';
const ZWNJ: char = '\u{200C}';

#[test]
fn letters_take_their_joining_forms() {
  let Some((fonts, font)) = fonts() else {
    return;
  };
  // Isolated.
  assert_eq!(shown(&fonts, &BEH.to_string()), [form(&font, 0xFE8F)]);
  // Alef joins only the letter before it, so the beh after it starts
  // again: beh initial, alef final, beh isolated, shown right to left.
  let word: String = [BEH, ALEF, BEH].iter().collect();
  assert_eq!(
    shown(&fonts, &word),
    [
      form(&font, 0xFE8F),
      form(&font, 0xFE8E),
      form(&font, 0xFE91)
    ]
  );
  // Seen initial and meem final.
  let word: String = [SEEN, MEEM].iter().collect();
  assert_eq!(
    shown(&fonts, &word),
    [form(&font, 0xFEE2), form(&font, 0xFEB3)]
  );
}

/// Words as Uniscribe shaped them in Segoe UI 5.72, left to right as
/// shown, marks included.
const UNISCRIBE: [(&str, &[u16]); 7] = [
  ("\u{0628}", &[2318]),
  ("\u{0628}\u{0628}\u{0628}", &[3622, 3623, 2251]),
  ("\u{0628}\u{0627}\u{0628}", &[2318, 2247, 2251]),
  ("\u{0644}\u{0627}", &[3941, 3980]),
  ("\u{0628}\u{0644}\u{0627}", &[3941, 3981, 2251]),
  (
    "\u{0633}\u{0644}\u{0627}\u{0645}",
    &[2342, 3941, 3981, 2260],
  ),
  ("\u{0628}\u{064E}\u{0628}", &[3622, 2351, 2251]),
];

#[test]
fn words_match_windows_own_shaper() {
  let Some((fonts, font)) = fonts() else {
    return;
  };
  if font.glyph_count() != 5395 {
    println!("skipped: this Segoe UI is not revision 5.72");
    return;
  }
  for (word, want) in UNISCRIBE {
    let got: Vec<u16> = fonts
      .shape(word, 20.0)
      .glyphs
      .iter()
      .map(|g| g.glyph)
      .collect();
    assert_eq!(got, want, "{word:?}");
  }
}

#[test]
fn lam_and_alef_join_as_one() {
  let Some((fonts, _)) = fonts() else {
    return;
  };
  let alone: String = [LAM, ALEF].iter().collect();
  let apart: String = [LAM, ZWNJ, ALEF].iter().collect();
  // With a zero-width non-joiner between them they do not join, so the
  // glyphs differ.
  assert_ne!(shown(&fonts, &alone), shown(&fonts, &apart));
  // The two characters share the ligature's width for carets.
  let line = fonts.shape(&alone, 20.0);
  assert_eq!(line.clusters.len(), 2);
  let (lam, alef) = (line.clusters[0], line.clusters[1]);
  assert!(
    lam.left > alef.left,
    "lam is on the right: {lam:?} {alef:?}"
  );
}

#[test]
fn marks_sit_on_their_letter_and_do_not_break_joining() {
  let Some((fonts, font)) = fonts() else {
    return;
  };
  let word: String = [BEH, FATHA, BEH].iter().collect();
  assert_eq!(
    shown(&fonts, &word),
    [form(&font, 0xFE90), form(&font, 0xFE91)]
  );
  let line = fonts.shape(&word, 20.0);
  let mark = line
    .glyphs
    .iter()
    .find(|g| g.advance == 0.0)
    .expect("the fatha has a glyph");
  let first = line.glyphs.iter().find(|g| g.byte == 0).unwrap();
  let outline = font.outline(mark.glyph);
  let bottom = outline.bounds.0.y * 20.0 / font.units_per_em() + mark.y;
  assert!(bottom > 0.0, "the fatha is drawn above the line: {mark:?}");
  assert!(mark.y != 0.0, "the fatha was moved onto its letter");
  assert!(
    mark.x >= first.x - 2.0 && mark.x <= first.x + first.advance + 2.0,
    "the fatha is over the first beh: {mark:?} {first:?}"
  );
}

#[test]
fn mixed_lines_show_each_run_in_order() {
  let Some((fonts, _)) = fonts() else {
    return;
  };
  // English, then Arabic: the Arabic word reads right to left, after the
  // English on the left.
  let text = format!("ab {SEEN}{LAM}{ALEF}{MEEM} cd");
  let line = fonts.shape(&text, 20.0);
  assert!(!line.rtl);
  let bytes: Vec<usize> = line.glyphs.iter().map(|g| g.byte).collect();
  let arabic = text.find(SEEN).unwrap();
  let meem = text.find(MEEM).unwrap();
  assert_eq!(&bytes[..3], [0, 1, 2], "ab and the space first");
  assert_eq!(bytes[3], meem, "then the Arabic, from its last letter");
  assert_eq!(*bytes.last().unwrap(), text.len() - 1, "d last");
  assert!(bytes.contains(&arabic));
  // A paragraph that starts in Arabic runs right to left.
  let rtl = format!("{SEEN}{LAM}{ALEF}{MEEM} ab");
  let line = fonts.shape(&rtl, 20.0);
  assert!(line.rtl);
  assert_eq!(
    line.glyphs[0].byte,
    rtl.find('a').unwrap(),
    "ab on the left"
  );
}

#[test]
fn the_caret_and_selection_follow_the_text() {
  let Some((fonts, _)) = fonts() else {
    return;
  };
  let text = format!("ab {BEH}{BEH}{BEH} cd");
  let line = fonts.shape(&text, 20.0);
  let b1 = text.find(BEH).unwrap();
  let b2 = b1 + 2;
  let b3 = b1 + 4;
  let after = b1 + 6;
  // After the first beh, the caret is on its left: inside the Arabic.
  let x1 = line.caret_x(b2);
  let x2 = line.caret_x(b3);
  let x3 = line.caret_x(after);
  assert!(
    x1 > x2 && x2 > x3,
    "the caret moves left through Arabic: {x1} {x2} {x3}"
  );
  // Right arrow, from the end of "ab ", goes into the Arabic from the
  // left: to after its last letter.
  assert_eq!(line.move_visually(b1, true), Some(after));
  // Every caret place can be reached with Right, and only once.
  let mut at = 0;
  let mut seen = vec![at];
  while let Some(next) = line.move_visually(at, true) {
    assert!(!seen.contains(&next), "{next} twice: {seen:?}");
    seen.push(next);
    at = next;
  }
  assert_eq!(seen.len(), line.caret_stops().len());
  // A selection across the direction change is two pieces.
  let pieces = line.selection(1, b2);
  assert_eq!(pieces.len(), 2, "{pieces:?}");
  // Clicking each caret place finds a place there.
  for (byte, x) in line.caret_stops() {
    let found = line.byte_at(x);
    assert_eq!(line.caret_x(found), x, "a click at {x} for {byte}");
  }
}
