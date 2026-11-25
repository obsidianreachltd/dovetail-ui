//! Fonts, glyph rasterising, font fallback and text shaping for Dovetail UI
//! (M06). Everything here is Dovetail's own code ([D0002]).
//!
//! - [`Font`] reads a TrueType font file or one font of a collection
//!   (`.ttc`): metrics, the character map, advances and glyph outlines.
//! - [`rasterize`] turns a glyph outline into anti-aliased coverage.
//! - [`Fonts`] is an ordered list of fonts: the first is the main font, the
//!   rest are used, in order, for characters the main font does not have.
//!   [`Fonts::system`] finds the operating system's fonts.
//! - [`Fonts::shape`] places the glyphs of one line of text.
//!
//! - Arabic is shaped with `GSUB` and `GPOS` marks, and text that mixes
//!   directions is put in order by the bidirectional algorithm (M13,
//!   D0018).
//!
//! [D0002]: ../../../docs/decisions/D0002-build-own-stack.md

mod bidi;
mod cff;
mod edit;
mod font;
mod fonts;
mod kern;
mod language;
mod otl;
mod outline;
mod raster;
mod read;
mod shaping;
mod unicode;

pub use bidi::{
  BidiLevels, bracket_key, paragraph_level, resolve as resolve_bidi,
  visual_order,
};
pub use edit::TextBuffer;
pub use font::{Font, FontError, LineMetrics};
pub use fonts::{ClusterPlace, Fonts, ShapedGlyph, ShapedLine};
pub use language::Language;
pub use outline::{Outline, Point, Segment};
pub use raster::{GlyphBitmap, embolden, rasterize};
pub use unicode::{
  BidiClass, JoiningType, UNICODE_VERSION, bidi_class, grapheme_boundaries,
  joining_type, mirror, next_grapheme, previous_grapheme,
};
