//! Images and icons for Dovetail UI (M17, D0023).
//!
//! - [`Bitmap`] is a decoded image: PNG, JPEG and GIF files are read by
//!   Dovetail's own decoders, with no dependencies.
//! - [`Animation`] is every frame of a GIF or APNG file (D0024).
//! - [`Icon`] is an outline read from a small subset of SVG, drawn in one
//!   colour at the size it is shown. [`icons`] is Dovetail's own set.
//! - Both become a [`dovetail_core::Picture`] to show in a view.

extern crate dovetail_core as dovetail;

mod animation;
mod bitmap;
mod gif;
mod icon;
pub mod icons;
mod inflate;
mod jpeg;
mod path;
mod png;
mod raster;
mod svg;
mod xml;

pub use animation::{Animation, Frame, WeakAnimation};
pub use bitmap::{Bitmap, Level, MAX_TEXTURE, WeakBitmap};
pub use gif::{GifFrame, encode as encode_gif};
pub use icon::{Coverage, Icon};
pub use png::{ApngFrame, encode_apng};
pub use svg::SvgError;

/// The most pixels an image file may have: 128 million, half a gigabyte
/// once decoded.
pub const MAX_PIXELS: u64 = 1 << 27;

/// Pixels as four bytes each, red, green, blue and alpha, row by row, in
/// sRGB and not premultiplied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
  /// Width in pixels.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
  /// Four bytes for each pixel.
  pub pixels: Vec<u8>,
}

/// Why an image could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
  /// The data is neither PNG nor JPEG.
  NotAnImage,
  /// The data ends too soon.
  Truncated,
  /// The data breaks the format's rules.
  Invalid(&'static str),
  /// The data uses a part of the format that is not read.
  Unsupported(&'static str),
  /// The image is larger than [`MAX_PIXELS`].
  TooLarge,
  /// The file could not be read.
  Io(String),
}

impl std::fmt::Display for Error {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    match self {
      Error::NotAnImage => f.write_str("not a PNG, JPEG or GIF image"),
      Error::Truncated => f.write_str("the image ends too soon"),
      Error::Invalid(what) => write!(f, "a broken image: {what}"),
      Error::Unsupported(what) => write!(f, "not supported: {what}"),
      Error::TooLarge => f.write_str("the image is too large"),
      Error::Io(e) => write!(f, "the file could not be read: {e}"),
    }
  }
}

impl std::error::Error for Error {}

/// Decodes PNG or JPEG data into straight sRGB pixels, without making a
/// [`Bitmap`].
pub fn decode_rgba(data: &[u8]) -> Result<Rgba, Error> {
  if png::is_png(data) {
    png::decode(data)
  } else if jpeg::is_jpeg(data) {
    jpeg::decode(data)
  } else if gif::is_gif(data) {
    // The first frame.
    Ok(gif::decode(data)?.0.swap_remove(0).0)
  } else {
    Err(Error::NotAnImage)
  }
}

/// Writes straight sRGB pixels as a PNG file, stored rather than
/// compressed: for tests and tools, such as saving what was painted.
pub fn encode_png(image: &Rgba) -> Vec<u8> {
  png::encode(image)
}

/// Decodes a JPEG file as [`decode_rgba`] does, but with subsampled colour
/// taken from the nearest sample instead of blended, as some other
/// decoders do. For comparing with their output in tests.
#[doc(hidden)]
pub fn decode_jpeg_nearest(data: &[u8]) -> Result<Rgba, Error> {
  jpeg::decode_with(data, true)
}

use dovetail::{Picture, PictureKind, View};

impl Bitmap {
  /// The image as a picture to show, one image pixel to one logical pixel.
  pub fn picture(&self) -> Picture {
    self.picture_at(1.0)
  }

  /// The image as a picture with `density` image pixels to each logical
  /// pixel: 2 for an image drawn for 200% displays, which is then shown at
  /// half its pixel size, and pixel for pixel at 200%.
  pub fn picture_at(
    &self,
    density: f32,
  ) -> Picture {
    let d = density.max(1e-3);
    Picture::new(
      self.clone(),
      self.id(),
      PictureKind::Image {
        width: self.width() as f32 / d,
        height: self.height() as f32 / d,
      },
    )
  }
}

impl Icon {
  /// The icon as a picture to show, as tall as the text around it makes it
  /// and in its colour, as decoration.
  pub fn picture(&self) -> Picture {
    Picture::new(
      self.clone(),
      self.id(),
      PictureKind::Icon { aspect: self.aspect() },
    )
  }
}

impl From<Bitmap> for View {
  fn from(bitmap: Bitmap) -> View {
    View::Picture(bitmap.picture())
  }
}

impl From<&Icon> for View {
  fn from(icon: &Icon) -> View {
    View::Picture(icon.picture())
  }
}

impl From<Icon> for View {
  fn from(icon: Icon) -> View {
    View::Picture(icon.picture())
  }
}
