//! Unicode text segmentation, bidirectional classes and Arabic joining,
//! from tables generated out of the Unicode Character Database (D0014).

pub(crate) mod bidi;
mod bidi_tables;
mod grapheme;
mod tables;

pub use bidi::{
  BidiClass, JoiningType, bidi_class, bracket, joining_type, mirror,
};
pub use grapheme::{grapheme_boundaries, next_grapheme, previous_grapheme};
pub use tables::UNICODE_VERSION;
