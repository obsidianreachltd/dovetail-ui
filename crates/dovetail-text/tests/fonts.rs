//! Font reading, rasterising, fallback and shaping (M06 "Done when" item 1,
//! for Latin and CJK).
//!
//! The first tests build a tiny TrueType font in code, so every value is
//! known. The others read this machine's system fonts and are skipped
//! when a font is not installed.

use std::path::{Path, PathBuf};

use dovetail_text::{Font, FontError, Fonts, Point, rasterize};

// --- A tiny font, built here ------------------------------------------------
// --- A tiny font, built in `common` -----------------------------------------

mod common;

use common::{be16, sfnt, sfnt_with};

fn test_font() -> Font {
  Font::from_data(sfnt(0), 0).expect("the test font")
}

#[test]
fn reads_names_metrics_and_the_character_map() {
  let font = test_font();
  assert_eq!(font.family(), "Test Sans");
  assert_eq!(font.units_per_em(), 1000.0);
  assert_eq!(font.glyph_count(), 3);
  assert_eq!(font.glyph_index('A'), Some(1));
  assert_eq!(font.glyph_index('B'), Some(2));
  assert_eq!(font.glyph_index('C'), None);
  assert_eq!(font.advance(1), 600.0);
  let m = font.line_metrics(20.0);
  assert_eq!((m.ascent, m.descent, m.line_gap), (16.0, 4.0, 2.0));
  assert_eq!(m.line_height(), 22.0);
}

#[test]
fn reads_simple_and_composite_outlines() {
  let font = test_font();
  let square = font.outline(1);
  assert_eq!(square.segments.len(), 4);
  assert_eq!(
    square.bounds,
    (Point::new(100.0, 0.0), Point::new(500.0, 700.0))
  );
  let moved = font.outline(2);
  assert_eq!(moved.segments.len(), 4);
  assert_eq!(
    moved.bounds,
    (Point::new(1100.0, 100.0), Point::new(1500.0, 800.0))
  );
  assert!(font.outline(0).is_empty());
}

#[test]
fn rasterises_a_glyph_to_its_exact_pixels() {
  let font = test_font();
  // At 10 pixels per em the square is x 1..5, y 0..7: 4 × 7 full pixels.
  let g = rasterize(&font.outline(1), 0.01, 0.0);
  assert_eq!((g.width, g.height, g.left, g.top), (4, 7, 1, 7));
  assert!(g.coverage.iter().all(|&c| c == 255));
}

#[test]
fn reads_a_font_from_a_collection() {
  let mut ttc = Vec::new();
  ttc.extend(*b"ttcf");
  ttc.extend(0x0001_0000u32.to_be_bytes());
  ttc.extend(1u32.to_be_bytes()); // one font
  ttc.extend(16u32.to_be_bytes()); // at byte 16
  ttc.extend(sfnt(16));
  let font = Font::from_data(ttc.clone(), 0).expect("font 0 of the collection");
  assert_eq!(font.glyph_index('A'), Some(1));
  assert_eq!(
    Font::from_data(ttc, 1).unwrap_err(),
    FontError::NoSuchFont(1)
  );
}

#[test]
fn rejects_data_that_is_not_a_font_without_panicking() {
  assert_eq!(
    Font::from_data(&b"hello"[..], 0).unwrap_err(),
    FontError::NotAFont
  );
  assert_eq!(
    Font::from_data(&[][..], 0).unwrap_err(),
    FontError::NotAFont
  );
  // A real header with its tables cut off.
  let cut: Vec<u8> = sfnt(0)[..40].to_vec();
  assert!(Font::from_data(cut, 0).is_err());
  // Every truncation of the font fails cleanly or reads fine.
  let full = sfnt(0);
  for len in (0..full.len()).step_by(7) {
    let _ = Font::from_data(full[..len].to_vec(), 0).map(|f| f.outline(1));
  }
}

#[test]
fn shapes_with_fallback_to_a_second_font() {
  // The main font has only 'A' and 'B'; the second is the same font, so
  // "AC" places 'A' from font 0 and 'C' as font 0's missing glyph.
  let fonts = Fonts::from_fonts([test_font(), test_font()]);
  let line = fonts.shape("AB C", 10.0);
  let placed: Vec<(usize, u16, f32)> = line
    .glyphs
    .iter()
    .map(|g| (g.font, g.glyph, g.x))
    .collect();
  assert_eq!(
    placed,
    [
      (0, 1, 0.0),
      (0, 2, 6.0),
      (0, 0, 22.0),
      (0, 0, 27.0)
    ]
  );
  assert_eq!(line.width, 32.0);
  assert_eq!(line.metrics.line_height(), 11.0);
}

// --- This machine's fonts ---------------------------------------------------

fn system_font(path: &str) -> Option<Font> {
  let data = std::fs::read(Path::new(path)).ok()?;
  Some(Font::from_data(data, 0).expect("a system font that reads"))
}

#[test]
fn reads_the_windows_ui_font() {
  let Some(font) = system_font("C:/Windows/Fonts/segoeui.ttf") else {
    return;
  };
  assert_eq!(font.family(), "Segoe UI");
  let a = font.glyph_index('A').expect("A");
  let outline = font.outline(a);
  assert!(outline.segments.len() > 8, "A has a real outline");
  let bitmap = rasterize(&outline, 20.0 / font.units_per_em(), 0.0);
  let ink = bitmap
    .coverage
    .iter()
    .filter(|&&c| c > 128)
    .count();
  assert!(ink > 20, "A at 20 pixels has ink: {ink}");
  assert!(font.glyph_index('中').is_none(), "Segoe UI has no CJK");
}

#[test]
fn reads_a_cjk_font_from_a_collection() {
  let Some(font) = system_font("C:/Windows/Fonts/msyh.ttc") else {
    return;
  };
  let g = font
    .glyph_index('中')
    .expect("中 in Microsoft YaHei");
  assert!(!font.outline(g).is_empty());
  assert!(font.glyph_index('文').is_some());
}

#[test]
fn system_fonts_fall_back_for_cjk() {
  let fonts = Fonts::system();
  if fonts.is_empty() || !Path::new("C:/Windows/Fonts/msyh.ttc").is_file() {
    return;
  }
  let line = fonts.shape("Hello 中文", 16.0);
  assert_eq!(line.glyphs.len(), 8);
  let latin = line.glyphs[0].font;
  let cjk = line.glyphs[6].font;
  assert_ne!(latin, cjk, "CJK comes from a fallback font");
  assert!(
    line.glyphs.iter().all(|g| g.glyph != 0),
    "every character has a glyph"
  );
  assert!(
    line.width > 50.0 && line.width < 120.0,
    "width {}",
    line.width
  );
  assert!(
    line.metrics.line_height() > 16.0,
    "line height {}",
    line.metrics.line_height()
  );
}

#[test]
fn the_font_list_can_be_chosen_by_environment() {
  // Checked through `from_files`, which `DOVETAIL_FONTS` feeds; a missing
  // file is skipped, not an error.
  let fonts = Fonts::from_files([(PathBuf::from("no/such/font.ttf"), 0)]);
  assert!(fonts.is_empty());
  assert_eq!(fonts.shape("abc", 10.0).glyphs.len(), 0);
}

#[test]
fn caret_positions_and_clicks_map_to_graphemes() {
  let fonts = Fonts::from_fonts([test_font()]);
  // 'A' is 6 pixels wide and 'B' 16 at 10 pixels.
  assert_eq!(fonts.caret_x("AB", 10.0, 0), 0.0);
  assert_eq!(fonts.caret_x("AB", 10.0, 1), 6.0);
  assert_eq!(fonts.caret_x("AB", 10.0, 2), 22.0);
  assert_eq!(fonts.byte_at("AB", 10.0, 2.0), 0, "nearer the start");
  assert_eq!(
    fonts.byte_at("AB", 10.0, 5.0),
    1,
    "nearer the gap between A and B"
  );
  assert_eq!(fonts.byte_at("AB", 10.0, 100.0), 2, "past the end");
  let line = fonts.shape("AB", 10.0);
  assert_eq!(
    line
      .glyphs
      .iter()
      .map(|g| g.byte)
      .collect::<Vec<_>>(),
    [0, 1]
  );
}

// --- Kerning ------------------------------------------------------------------

fn i16be(v: i16) -> [u8; 2] {
  v.to_be_bytes()
}

/// A `GPOS` table with one `kern` feature using one lookup of
/// `lookup_type`, with one subtable.
fn gpos(
  lookup_type: u16,
  subtable: Vec<u8>,
) -> Vec<u8> {
  let mut t = Vec::new();
  t.extend(be16(1)); // version 1.0
  t.extend(be16(0));
  t.extend(be16(10)); // script list
  t.extend(be16(12)); // feature list
  t.extend(be16(26)); // lookup list
  t.extend(be16(0)); // no scripts
  // Feature list: one record, "kern", its feature 8 bytes on.
  t.extend(be16(1));
  t.extend(b"kern");
  t.extend(be16(8));
  t.extend(be16(0)); // feature params
  t.extend(be16(1)); // one lookup
  t.extend(be16(0)); // lookup 0
  // Lookup list: one lookup, 4 bytes on.
  t.extend(be16(1));
  t.extend(be16(4));
  t.extend(be16(lookup_type));
  t.extend(be16(0)); // flags
  t.extend(be16(1)); // one subtable
  t.extend(be16(8)); // 8 bytes on
  t.extend(subtable);
  t
}

/// Pair adjustment format 1: glyph 1 then glyph 2 moves by `value`.
fn pair_list(value: i16) -> Vec<u8> {
  let mut t = Vec::new();
  t.extend(be16(1)); // format
  t.extend(be16(12)); // coverage
  t.extend(be16(0x0004)); // first value: x advance
  t.extend(be16(0)); // second value: nothing
  t.extend(be16(1)); // one pair set
  t.extend(be16(18));
  // Coverage format 1: glyph 1.
  t.extend(be16(1));
  t.extend(be16(1));
  t.extend(be16(1));
  // Pair set: one record, second glyph 2.
  t.extend(be16(1));
  t.extend(be16(2));
  t.extend(i16be(value));
  t
}

/// Pair adjustment format 2: class 1 (glyph 1) then class 1 (glyph 2)
/// moves by `value`.
fn pair_classes(value: i16) -> Vec<u8> {
  let mut t = Vec::new();
  t.extend(be16(2)); // format
  t.extend(be16(24)); // coverage
  t.extend(be16(0x0004));
  t.extend(be16(0));
  t.extend(be16(34)); // first classes
  t.extend(be16(44)); // second classes
  t.extend(be16(2)); // two first classes
  t.extend(be16(2)); // two second classes
  for v in [0, 0, 0, value] {
    t.extend(i16be(v));
  }
  // Coverage format 2: glyphs 1 to 1.
  t.extend(be16(2));
  t.extend(be16(1));
  t.extend(be16(1));
  t.extend(be16(1));
  t.extend(be16(0));
  // First classes, format 2: glyph 1 is class 1.
  t.extend(be16(2));
  t.extend(be16(1));
  t.extend(be16(1));
  t.extend(be16(1));
  t.extend(be16(1));
  // Second classes, format 1: glyph 2 is class 1.
  t.extend(be16(1));
  t.extend(be16(2));
  t.extend(be16(1));
  t.extend(be16(1));
  t
}

/// An extension subtable holding a pair adjustment subtable.
fn extension(subtable: Vec<u8>) -> Vec<u8> {
  let mut t = Vec::new();
  t.extend(be16(1));
  t.extend(be16(2)); // pair adjustment
  t.extend(8u32.to_be_bytes());
  t.extend(subtable);
  t
}

/// A `kern` table, version 0, with one pair: glyph 1 then glyph 2.
fn kern_table(value: i16) -> Vec<u8> {
  let mut t = Vec::new();
  t.extend(be16(0)); // version
  t.extend(be16(1)); // one subtable
  t.extend(be16(0)); // subtable version
  t.extend(be16(14 + 6)); // length
  t.extend(be16(0x0001)); // format 0, horizontal
  t.extend(be16(1)); // one pair
  t.extend([0u8; 6]); // search fields
  t.extend(be16(1));
  t.extend(be16(2));
  t.extend(i16be(value));
  t
}

fn font_with(extra: Vec<([u8; 4], Vec<u8>)>) -> Font {
  Font::from_data(sfnt_with(0, extra), 0).expect("the test font")
}

#[test]
fn reads_kerning_from_gpos_pair_lists_and_classes() {
  let font = font_with(vec![(*b"GPOS", gpos(2, pair_list(-50)))]);
  assert!(font.has_kerning());
  assert_eq!(font.kerning(1, 2), -50.0);
  assert_eq!(font.kerning(2, 1), 0.0, "only the pair listed");
  assert_eq!(font.kerning(1, 1), 0.0);

  let font = font_with(vec![(*b"GPOS", gpos(2, pair_classes(-70)))]);
  assert_eq!(font.kerning(1, 2), -70.0);
  assert_eq!(font.kerning(2, 1), 0.0, "glyph 2 is not covered first");

  let font = font_with(vec![(*b"GPOS", gpos(9, extension(pair_classes(-70))))]);
  assert_eq!(font.kerning(1, 2), -70.0, "through an extension lookup");
}

#[test]
fn reads_kerning_from_the_old_kern_table() {
  let font = font_with(vec![(*b"kern", kern_table(-30))]);
  assert_eq!(font.kerning(1, 2), -30.0);
  assert_eq!(font.kerning(2, 1), 0.0);
  // GPOS comes first when a font has both.
  let font = font_with(vec![
    (*b"kern", kern_table(-30)),
    (*b"GPOS", gpos(2, pair_list(-50))),
  ]);
  assert_eq!(font.kerning(1, 2), -50.0);
  assert!(!test_font().has_kerning());
}

#[test]
fn bad_kerning_data_is_ignored_without_panicking() {
  for cut in 0..60 {
    let mut table = gpos(2, pair_list(-50));
    table.truncate(cut);
    let font = font_with(vec![(*b"GPOS", table)]);
    let _ = font.kerning(1, 2);
  }
  let font = font_with(vec![(*b"GPOS", vec![0xff; 40])]);
  assert_eq!(font.kerning(1, 2), 0.0);
}

#[test]
fn shaping_applies_kerning_between_glyphs() {
  let fonts = Fonts::from_fonts([font_with(vec![(
    *b"GPOS",
    gpos(2, pair_list(-100)),
  )])]);
  // 1000 units per em at 10 pixels: 0.01 pixels per unit.
  let line = fonts.shape("AB", 10.0);
  assert_eq!(line.glyphs[0].advance, 6.0 - 1.0);
  assert_eq!(line.glyphs[1].x, 5.0);
  assert_eq!(line.width, 5.0 + 16.0);
  assert_eq!(fonts.caret_x("AB", 10.0, 1), 5.0, "the caret follows");
}

#[test]
fn system_fonts_kern_av_closer() {
  for path in [
    "C:/Windows/Fonts/segoeui.ttf",
    "C:/Windows/Fonts/arial.ttf",
  ] {
    let Some(font) = system_font(path) else {
      continue;
    };
    let (a, v) = (
      font.glyph_index('A').unwrap(),
      font.glyph_index('V').unwrap(),
    );
    assert!(font.kerning(a, v) < 0.0, "{path}: A then V move closer");
    let fonts = Fonts::from_fonts([font.clone()]);
    let pair = fonts.shape("AV", 20.0).width;
    let apart = fonts.shape("A", 20.0).width + fonts.shape("V", 20.0).width;
    assert!(
      pair < apart,
      "{path}: \"AV\" is narrower than A and V apart"
    );
  }
}

#[test]
fn han_characters_use_the_font_for_their_language() {
  use dovetail_text::Language;
  let files = [
    "C:/Windows/Fonts/segoeui.ttf",
    "C:/Windows/Fonts/msyh.ttc",
    "C:/Windows/Fonts/msjh.ttc",
    "C:/Windows/Fonts/YuGothR.ttc",
    "C:/Windows/Fonts/malgun.ttf",
  ];
  if !files.iter().all(|f| Path::new(f).is_file()) {
    eprintln!("skipped: the Windows CJK fonts are not installed");
    return;
  }
  let fonts = Fonts::from_files(files.iter().map(|f| (PathBuf::from(f), 0)));
  let font_of = |text: &str, i: usize| fonts.shape(text, 20.0).glyphs[i].font;

  // With no language, the first font that has it, as before.
  assert_eq!(font_of("直", 0), 1, "Microsoft YaHei");
  fonts.set_language(Language::Japanese);
  assert_eq!(font_of("直", 0), 3, "Yu Gothic");
  assert_eq!(font_of("。", 0), 3, "CJK punctuation too");
  fonts.set_language(Language::ChineseTraditional);
  assert_eq!(font_of("直", 0), 2, "Microsoft JhengHei");
  fonts.set_language(Language::Korean);
  let malgun = fonts.font(4).unwrap();
  if malgun.glyph_index('直').is_some() {
    assert_eq!(font_of("直", 0), 4, "Malgun Gothic");
  }
  // Kana in the text make it Japanese, whatever the list's language.
  fonts.set_language(Language::ChineseSimplified);
  assert_eq!(font_of("直", 0), 1);
  assert_eq!(font_of("直す", 0), 3, "Japanese, from the kana");
  // Latin text is not affected.
  assert_eq!(font_of("A直す", 0), 0);
}
