//! An ordered list of fonts with fallback, and shaping one line of text,
//! with kerning between glyphs of the same font.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crate::font::{Font, LineMetrics};
use crate::language::{Language, varies_by_language};
use crate::raster::{GlyphBitmap, rasterize};

pub use crate::shaping::{ClusterPlace, ShapedGlyph, ShapedLine};

/// Where a font comes from, before it is read.
#[derive(Clone, Debug)]
struct Source {
  path: PathBuf,
  index: u32,
}

#[derive(Default)]
struct Inner {
  sources: Vec<Source>,
  /// `None`: not read yet. `Some(None)`: could not be read.
  fonts: Vec<Option<Option<Rc<Font>>>>,
  /// The font chosen for each character in each language, once looked up.
  chosen: HashMap<(char, Language), (usize, u16)>,
  /// The language of text that does not show its own.
  language: Language,
  /// Kerning for each pair of glyphs in a font, in font units, once looked
  /// up.
  kerned: HashMap<(usize, u16, u16), f32>,
  /// Lines shaped lately, by text and size.
  shaped: HashMap<(String, u32), ShapedLine>,
  /// How much glyphs are thickened, as a share of the font size, for a
  /// weight with no font file of its own (M14).
  embolden: f32,
}

/// How many shaped lines are kept before the cache starts again.
const SHAPED_LINES_KEPT: usize = 512;

/// The fonts as shaping sees them, for text in one language.
struct Shaping<'a> {
  inner: &'a mut Inner,
  language: Language,
}

impl crate::shaping::FontSource for Shaping<'_> {
  fn choose(
    &mut self,
    c: char,
  ) -> Option<(usize, u16)> {
    self.inner.choose(c, self.language)
  }

  fn font(
    &mut self,
    i: usize,
  ) -> Option<Rc<Font>> {
    if i < self.inner.fonts.len() {
      self.inner.font(i)
    } else {
      None
    }
  }

  fn kerning(
    &mut self,
    i: usize,
    left: u16,
    right: u16,
  ) -> f32 {
    if let Some(&k) = self.inner.kerned.get(&(i, left, right)) {
      return k;
    }
    let k = self
      .inner
      .font(i)
      .map_or(0.0, |f| f.kerning(left, right));
    self.inner.kerned.insert((i, left, right), k);
    k
  }
}

impl Inner {
  fn font(
    &mut self,
    i: usize,
  ) -> Option<Rc<Font>> {
    if self.fonts[i].is_none() {
      let source = &self.sources[i];
      let loaded = std::fs::read(&source.path)
        .ok()
        .and_then(|data| Font::from_data(data, source.index).ok());
      self.fonts[i] = Some(loaded.map(Rc::new));
    }
    self.fonts[i].clone().flatten()
  }

  /// The first font that can be read: the main font.
  fn main(&mut self) -> Option<(usize, Rc<Font>)> {
    (0..self.fonts.len()).find_map(|i| self.font(i).map(|f| (i, f)))
  }

  /// The font and glyph for `c` in text in `language`: the first font
  /// that has it, or the main font's "missing" glyph. For Han characters
  /// and CJK punctuation, a font made for the language comes first.
  fn choose(
    &mut self,
    c: char,
    language: Language,
  ) -> Option<(usize, u16)> {
    let language = if varies_by_language(c) {
      language
    } else {
      Language::Other
    };
    if let Some(&chosen) = self.chosen.get(&(c, language)) {
      return Some(chosen);
    }
    let find = |inner: &mut Inner, for_language: bool| {
      (0..inner.fonts.len()).find_map(|i| {
        inner
          .font(i)
          .filter(|f| !for_language || f.is_for(language))
          .and_then(|f| f.glyph_index(c))
          .map(|g| (i, g))
      })
    };
    let found = if language == Language::Other {
      find(self, false)
    } else {
      find(self, true).or_else(|| find(self, false))
    };
    let chosen = match found {
      Some(found) => found,
      None => (self.main()?.0, 0),
    };
    self.chosen.insert((c, language), chosen);
    Some(chosen)
  }
}

/// An ordered list of fonts: the first that can be read is the main font,
/// and the others are used, in order, for characters it does not have.
/// Fonts are read from their files only when first needed.
///
/// Cloning is cheap: clones share the fonts.
#[derive(Clone, Default)]
pub struct Fonts {
  inner: Rc<RefCell<Inner>>,
}

thread_local! {
  static SYSTEM: Fonts = {
    let fonts = Fonts::from_files(system_font_files());
    if let Ok(tag) = std::env::var("DOVETAIL_LANG") {
      fonts.set_language(Language::from_tag(&tag));
    }
    fonts
  };
}

impl Fonts {
  /// Fonts read from files, in order: a path and the font's number in it (0
  /// unless it is a collection).
  pub fn from_files(files: impl IntoIterator<Item = (PathBuf, u32)>) -> Fonts {
    let sources: Vec<Source> = files
      .into_iter()
      .map(|(path, index)| Source { path, index })
      .collect();
    let fonts = vec![None; sources.len()];
    Fonts {
      inner: Rc::new(RefCell::new(Inner {
        sources,
        fonts,
        chosen: HashMap::new(),
        kerned: HashMap::new(),
        shaped: HashMap::new(),
        embolden: 0.0,
        language: Language::Other,
      })),
    }
  }

  /// Fonts already read, in order.
  pub fn from_fonts(fonts: impl IntoIterator<Item = Font>) -> Fonts {
    let fonts: Vec<Option<Option<Rc<Font>>>> = fonts
      .into_iter()
      .map(|f| Some(Some(Rc::new(f))))
      .collect();
    let sources = vec![Source { path: PathBuf::new(), index: 0 }; fonts.len()];
    Fonts {
      inner: Rc::new(RefCell::new(Inner {
        sources,
        fonts,
        chosen: HashMap::new(),
        kerned: HashMap::new(),
        shaped: HashMap::new(),
        embolden: 0.0,
        language: Language::Other,
      })),
    }
  }

  /// The operating system's fonts: its user-interface font first, then
  /// fonts for other scripts. Shared by everything on this thread.
  ///
  /// The environment variable `DOVETAIL_FONTS` replaces the list: font file
  /// paths separated by the system's path separator (`;` on Windows, `:`
  /// elsewhere).
  pub fn system() -> Fonts {
    SYSTEM.with(Fonts::clone)
  }

  /// The operating system's fonts for text of `weight`, from 100 (thin)
  /// to 900 (black), with 400 regular (M14). The system's font file for
  /// the nearest weight comes first, then the usual fonts. Where the
  /// nearest file is lighter than a bold weight asks for, glyphs are
  /// thickened. Shared by everything on this thread.
  pub fn system_weight(weight: u16) -> Fonts {
    if weight == 400 {
      return Fonts::system();
    }
    thread_local! {
      static WEIGHTS: RefCell<HashMap<u16, Fonts>> = RefCell::new(HashMap::new());
    }
    if let Some(f) = WEIGHTS.with_borrow(|w| w.get(&weight).cloned()) {
      return f;
    }
    let (first, embolden) = weighted_font_file(weight);
    let fonts = Fonts::from_files(first.into_iter().chain(system_font_files()));
    fonts.inner.borrow_mut().embolden = embolden;
    fonts.set_language(Fonts::system().language());
    WEIGHTS.with_borrow_mut(|w| w.insert(weight, fonts.clone()));
    fonts
  }

  /// How much glyphs are thickened, as a share of the font size: more
  /// than 0 for synthetic bold.
  pub fn embolden(&self) -> f32 {
    self.inner.borrow().embolden
  }

  /// Makes these fonts thicken glyphs by `share` of the font size, for a
  /// weight that has no font file.
  pub fn set_embolden(
    &self,
    share: f32,
  ) {
    self.inner.borrow_mut().embolden = share.max(0.0);
  }

  /// Sets the language of text that does not show its own (by kana or
  /// Hangul), which picks the font for Han characters. The desktop crate
  /// sets the system's; the environment variable `DOVETAIL_LANG` (such as
  /// `ja` or `zh-TW`) sets it first.
  pub fn set_language(
    &self,
    language: Language,
  ) {
    let mut inner = self.inner.borrow_mut();
    if inner.language != language {
      inner.language = language;
      inner.chosen.clear();
      inner.shaped.clear();
    }
  }

  /// The language of text that does not show its own.
  pub fn language(&self) -> Language {
    self.inner.borrow().language
  }

  /// True when no font in the list can be read.
  pub fn is_empty(&self) -> bool {
    self.inner.borrow_mut().main().is_none()
  }

  /// The main font's family name, or empty when there is none.
  pub fn main_family(&self) -> String {
    self
      .inner
      .borrow_mut()
      .main()
      .map(|(_, f)| f.family().to_owned())
      .unwrap_or_default()
  }

  /// Font number `i`, if it can be read.
  pub fn font(
    &self,
    i: usize,
  ) -> Option<Rc<Font>> {
    let mut inner = self.inner.borrow_mut();
    if i < inner.fonts.len() {
      inner.font(i)
    } else {
      None
    }
  }

  /// The main font's line metrics at `size` pixels.
  pub fn line_metrics(
    &self,
    size: f32,
  ) -> LineMetrics {
    self
      .inner
      .borrow_mut()
      .main()
      .map(|(_, f)| f.line_metrics(size))
      .unwrap_or_default()
  }

  /// Places the glyphs of `text`, on one line, at `size` pixels: one
  /// paragraph, shaped and put in visual order (M13).
  pub fn shape(
    &self,
    text: &str,
    size: f32,
  ) -> ShapedLine {
    let key = (text.to_owned(), size.to_bits());
    if let Some(line) = self.inner.borrow().shaped.get(&key) {
      return line.clone();
    }
    let mut inner = self.inner.borrow_mut();
    let metrics = inner
      .main()
      .map(|(_, f)| f.line_metrics(size))
      .unwrap_or_default();
    let language = Language::of_text(text).unwrap_or(inner.language);
    let line = if inner.main().is_some() {
      crate::shaping::shape(
        &mut Shaping { inner: &mut inner, language },
        text,
        size,
        metrics,
      )
    } else {
      ShapedLine {
        metrics,
        len: text.len(),
        ..ShapedLine::default()
      }
    };
    if inner.shaped.len() >= SHAPED_LINES_KEPT {
      inner.shaped.clear();
    }
    inner.shaped.insert(key, line.clone());
    line
  }

  /// The x position, in pixels from the line's left end, of the caret
  /// before byte `at` of one line of `text` (see
  /// [`ShapedLine::caret_x`]).
  pub fn caret_x(
    &self,
    text: &str,
    size: f32,
    at: usize,
  ) -> f32 {
    self.shape(text, size).caret_x(at)
  }

  /// The grapheme boundary of one line of `text` nearest to `x` pixels from
  /// the line's left end: where a click there puts the caret.
  pub fn byte_at(
    &self,
    text: &str,
    size: f32,
    x: f32,
  ) -> usize {
    self.shape(text, size).byte_at(x)
  }

  /// The pieces of one line of `text` that bytes `from..to` cover, from
  /// left to right (see [`ShapedLine::selection`]).
  pub fn selection(
    &self,
    text: &str,
    size: f32,
    from: usize,
    to: usize,
  ) -> Vec<(f32, f32)> {
    self.shape(text, size).selection(from, to)
  }

  /// The caret place one step left or right of `at` on the screen (see
  /// [`ShapedLine::move_visually`]).
  pub fn move_visually(
    &self,
    text: &str,
    size: f32,
    at: usize,
    right: bool,
  ) -> Option<usize> {
    self.shape(text, size).move_visually(at, right)
  }

  /// Draws a glyph at `size` pixels. `x_offset` is the pen's position inside
  /// its pixel, from 0 to 1.
  pub fn rasterize(
    &self,
    font: usize,
    glyph: u16,
    size: f32,
    x_offset: f32,
  ) -> GlyphBitmap {
    let bitmap = match self.font(font) {
      Some(f) => {
        rasterize(&f.outline(glyph), size / f.units_per_em(), x_offset)
      },
      None => GlyphBitmap::default(),
    };
    let embolden = self.embolden();
    if embolden > 0.0 {
      // Half on each side.
      crate::raster::embolden(&bitmap, embolden * size / 2.0)
    } else {
      bitmap
    }
  }
}

/// The system font file for the weight nearest `weight`, and how much to
/// thicken glyphs when that file is lighter than a bold weight asks for.
fn weighted_font_file(weight: u16) -> (Option<(PathBuf, u32)>, f32) {
  if std::env::var_os("DOVETAIL_FONTS").is_some() {
    return (None, synthetic(weight, 400));
  }
  // The main font's family, by weight.
  let files: &[(u16, &str)] = if cfg!(windows) {
    &[
      (300, "C:/Windows/Fonts/segoeuil.ttf"),
      (350, "C:/Windows/Fonts/segoeuisl.ttf"),
      (400, "C:/Windows/Fonts/segoeui.ttf"),
      (600, "C:/Windows/Fonts/seguisb.ttf"),
      (700, "C:/Windows/Fonts/segoeuib.ttf"),
      (900, "C:/Windows/Fonts/seguibl.ttf"),
    ]
  } else if cfg!(target_os = "macos") {
    // The system font is a variable font, which Dovetail cannot read by
    // weight yet: bold is made by thickening.
    &[]
  } else {
    &[
      (400, "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
      (700, "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
      (400, "/usr/share/fonts/TTF/DejaVuSans.ttf"),
      (700, "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf"),
      (400, "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf"),
      (
        700,
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
      ),
    ]
  };
  let found: Vec<(u16, &str)> = files
    .iter()
    .copied()
    .filter(|(_, p)| std::path::Path::new(p).is_file())
    .collect();
  // The nearest weight; for bold weights, a heavier file before a lighter
  // one, as CSS chooses.
  let best = found.iter().min_by_key(|(w, _)| {
    let distance = (i32::from(*w) - i32::from(weight)).unsigned_abs();
    let lighter = (*w < weight && weight >= 500) as u32;
    (lighter, distance)
  });
  match best {
    Some(&(w, path)) => (Some((PathBuf::from(path), 0)), synthetic(weight, w)),
    None => (None, synthetic(weight, 400)),
  }
}

/// How much to thicken glyphs from a file of weight `have` to show
/// `want`, as a share of the font size: none unless `want` is clearly
/// heavier.
fn synthetic(
  want: u16,
  have: u16,
) -> f32 {
  if want >= have + 150 {
    f32::from(want - have) / 300.0 * 0.04
  } else {
    0.0
  }
}

/// The font files to try, for this operating system, in order.
fn system_font_files() -> Vec<(PathBuf, u32)> {
  if let Some(list) = std::env::var_os("DOVETAIL_FONTS") {
    return std::env::split_paths(&list)
      .map(|p| (p, 0))
      .collect();
  }
  let candidates: &[(&str, u32)] = if cfg!(windows) {
    &[
      ("C:/Windows/Fonts/segoeui.ttf", 0),
      ("C:/Windows/Fonts/arial.ttf", 0),
      ("C:/Windows/Fonts/seguisym.ttf", 0),
      ("C:/Windows/Fonts/msyh.ttc", 0),
      ("C:/Windows/Fonts/msjh.ttc", 0),
      ("C:/Windows/Fonts/YuGothR.ttc", 0),
      ("C:/Windows/Fonts/malgun.ttf", 0),
      ("C:/Windows/Fonts/simsun.ttc", 0),
      ("C:/Windows/Fonts/tahoma.ttf", 0),
    ]
  } else if cfg!(target_os = "macos") {
    &[
      ("/System/Library/Fonts/SFNS.ttf", 0),
      ("/System/Library/Fonts/Helvetica.ttc", 0),
      // Arabic (M13).
      ("/System/Library/Fonts/GeezaPro.ttc", 0),
      ("/System/Library/Fonts/Supplemental/GeezaPro.ttc", 0),
      ("/Library/Fonts/Arial Unicode.ttf", 0),
      ("/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0),
      ("/System/Library/Fonts/PingFang.ttc", 0),
      ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0),
      ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
      ("/System/Library/Fonts/AppleSDGothicNeo.ttc", 0),
    ]
  } else {
    &[
      ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
      ("/usr/share/fonts/TTF/DejaVuSans.ttf", 0),
      ("/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf", 0),
      ("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", 0),
      // Arabic (M13).
      (
        "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
        0,
      ),
      ("/usr/share/fonts/noto/NotoSansArabic-Regular.ttf", 0),
      ("/usr/share/fonts/google-noto/NotoSansArabic-Regular.ttf", 0),
      (
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        0,
      ),
      (
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        0,
      ),
      ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
      // One collection with a face for each region: JP, KR, SC, TC.
      ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
      ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 1),
      ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 2),
      ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 3),
      (
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        0,
      ),
      (
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        1,
      ),
      (
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        2,
      ),
      (
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        3,
      ),
    ]
  };
  candidates
    .iter()
    .map(|&(path, index)| (PathBuf::from(path), index))
    .filter(|(path, _)| path.is_file())
    .collect()
}
