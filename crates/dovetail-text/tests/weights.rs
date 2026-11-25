//! Font weights (M14, D0019): the system's font file for a weight, and
//! synthetic bold where there is none.

use std::path::PathBuf;

use dovetail_text::{Font, Fonts, embolden, rasterize};

const SEGOE: &str = "C:/Windows/Fonts/segoeui.ttf";

/// Ink in a bitmap, in whole pixels' worth.
fn ink(b: &dovetail_text::GlyphBitmap) -> f32 {
  b.coverage
    .iter()
    .map(|&c| f32::from(c) / 255.0)
    .sum()
}

#[test]
fn bold_comes_from_its_own_font_file_where_there_is_one() {
  let bold = Fonts::system_weight(700);
  if cfg!(windows) && PathBuf::from("C:/Windows/Fonts/segoeuib.ttf").is_file() {
    assert_eq!(bold.main_family(), "Segoe UI");
    assert_eq!(bold.embolden(), 0.0, "a real bold font needs no thickening");
    let regular = Fonts::system();
    let w = |f: &Fonts| f.shape("Bold text", 20.0).width;
    assert!(w(&bold) > w(&regular), "bold letters are wider");
    // A file's own bold has more ink than its regular.
    let line = bold.shape("H", 40.0);
    let g = line.glyphs[0];
    let heavy = ink(&bold.rasterize(g.font, g.glyph, 40.0, 0.0));
    let line = regular.shape("H", 40.0);
    let g = line.glyphs[0];
    let light = ink(&regular.rasterize(g.font, g.glyph, 40.0, 0.0));
    assert!(heavy > light * 1.2, "{heavy} against {light}");
  }
  // Regular is the system's own list.
  assert_eq!(
    Fonts::system_weight(400).main_family(),
    Fonts::system().main_family()
  );
}

#[test]
fn synthetic_bold_thickens_glyphs_without_moving_the_pen() {
  let path = PathBuf::from(SEGOE);
  if !path.is_file() {
    println!("skipped: {SEGOE} is missing");
    return;
  }
  let font = Font::from_data(std::fs::read(&path).unwrap(), 0).unwrap();
  let g = font.glyph_index('l').unwrap();
  let plain = rasterize(&font.outline(g), 40.0 / font.units_per_em(), 0.0);
  let thick = embolden(&plain, 1.0);
  assert!(
    ink(&thick) > ink(&plain) * 1.3,
    "{} against {}",
    ink(&thick),
    ink(&plain)
  );
  assert_eq!(
    thick.width,
    plain.width + 4,
    "one pixel and a margin on each side"
  );
  assert_eq!(thick.left, plain.left - 2);
  assert_eq!(embolden(&plain, 0.0), plain);

  // Fonts that thicken: the same advances, more ink.
  let fonts = Fonts::from_files([(path.clone(), 0)]);
  let synthetic = Fonts::from_files([(path, 0)]);
  synthetic.set_embolden(0.04);
  assert_eq!(
    fonts.shape("Hello", 20.0).width,
    synthetic.shape("Hello", 20.0).width
  );
  let a = ink(&fonts.rasterize(0, g, 20.0, 0.0));
  let b = ink(&synthetic.rasterize(0, g, 20.0, 0.0));
  assert!(b > a, "{b} against {a}");
}
