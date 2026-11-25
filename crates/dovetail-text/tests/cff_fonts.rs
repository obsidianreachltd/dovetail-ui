//! CFF outlines in real fonts (M06 step 3). These read two open-licence
//! Adobe fonts downloaded into `tools/fonts/` (which git ignores), and are
//! skipped when the files are missing:
//!
//! - `SourceSans3-Regular.otf`, an ordinary CFF font;
//! - `SourceHanSansJP-Regular.otf`, a CID-keyed CJK font.

use std::path::PathBuf;

use dovetail_text::{Font, Fonts, GlyphBitmap, Segment};

fn test_font(name: &str) -> Option<Font> {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../tools/fonts")
    .join(name);
  let Ok(data) = std::fs::read(&path) else {
    eprintln!("skipped: {} is not there", path.display());
    return None;
  };
  Some(Font::from_data(data, 0).expect("the font reads"))
}

/// The runs of dark pixels along a row or column.
fn runs(pixels: impl Iterator<Item = u8>) -> usize {
  let mut count = 0;
  let mut dark = false;
  for p in pixels {
    let now = p > 128;
    if now && !dark {
      count += 1;
    }
    dark = now;
  }
  count
}

fn row(
  b: &GlyphBitmap,
  y: u32,
) -> impl Iterator<Item = u8> + '_ {
  (0..b.width).map(move |x| b.coverage[(y * b.width + x) as usize])
}

fn column(
  b: &GlyphBitmap,
  x: u32,
) -> impl Iterator<Item = u8> + '_ {
  (0..b.height).map(move |y| b.coverage[(y * b.width + x) as usize])
}

fn draw(
  font: &Font,
  c: char,
  size: f32,
) -> GlyphBitmap {
  let glyph = font.glyph_index(c).expect("the font has it");
  dovetail_text::rasterize(
    &font.outline(glyph),
    size / font.units_per_em(),
    0.0,
  )
}

#[test]
fn reads_an_ordinary_cff_font() {
  let Some(font) = test_font("SourceSans3-Regular.otf") else {
    return;
  };
  assert_eq!(font.family(), "Source Sans 3");
  let a = font.outline(font.glyph_index('A').unwrap());
  assert!(!a.is_empty());
  assert!(
    a.segments
      .iter()
      .all(|s| matches!(s, Segment::Line(..) | Segment::Cubic(..)))
  );
  let (min, max) = a.bounds;
  assert!(
    min.x >= -50.0 && max.x <= 1000.0 && min.y >= -50.0 && max.y <= 1000.0
  );

  // "o" is a ring: two walls across its middle, top and bottom down it.
  let o = draw(&font, 'o', 48.0);
  assert_eq!(runs(row(&o, o.height / 2)), 2, "across the o");
  assert_eq!(runs(column(&o, o.width / 2)), 2, "down the o");
  // "H" has two stems, and one bar down the middle.
  let h = draw(&font, 'H', 48.0);
  assert_eq!(runs(row(&h, h.height / 4)), 2);
  assert_eq!(runs(column(&h, h.width / 2)), 1);
}

#[test]
fn reads_a_cid_keyed_cjk_font() {
  let Some(font) = test_font("SourceHanSansJP-Regular.otf") else {
    return;
  };
  // Made for Japanese, by its name, though its code pages also claim
  // Simplified Chinese.
  assert!(font.is_for(dovetail_text::Language::Japanese));
  assert!(!font.is_for(dovetail_text::Language::ChineseSimplified));
  // 日: a box with a bar across the middle.
  let sun = draw(&font, '日', 64.0);
  assert_eq!(runs(column(&sun, sun.width / 2)), 3, "top, middle, bottom");
  assert_eq!(runs(row(&sun, sun.height / 4)), 2, "left and right walls");
  // 本 and 語 draw too, from other groups of glyphs.
  for c in ['本', '語', 'あ', 'ア', 'A'] {
    let glyph = font.glyph_index(c).expect("the font has it");
    assert!(!font.outline(glyph).is_empty(), "{c} has an outline");
  }
}

#[test]
fn cff_fonts_shape_and_fall_back() {
  let (Some(latin), Some(cjk)) = (
    test_font("SourceSans3-Regular.otf"),
    test_font("SourceHanSansJP-Regular.otf"),
  ) else {
    return;
  };
  let fonts = Fonts::from_fonts([latin, cjk]);
  let line = fonts.shape("AV 日本", 20.0);
  let used: Vec<usize> = line.glyphs.iter().map(|g| g.font).collect();
  assert_eq!(used, [0, 0, 0, 1, 1]);
  assert!(
    line.glyphs[0].advance < fonts.shape("A", 20.0).width,
    "kerned"
  );
}

#[test]
fn every_glyph_of_both_fonts_reads_without_panicking() {
  for name in [
    "SourceSans3-Regular.otf",
    "SourceHanSansJP-Regular.otf",
  ] {
    let Some(font) = test_font(name) else {
      continue;
    };
    let empty = (0..font.glyph_count())
      .filter(|&g| font.outline(g).is_empty())
      .count();
    // Spaces and similar draw nothing; nearly every glyph draws something.
    assert!(
      empty * 100 < font.glyph_count() as usize,
      "{name}: {empty} of {} glyphs are empty",
      font.glyph_count()
    );
  }
}
