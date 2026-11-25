//! Decoded images, ready for the GPU (D0023).
//!
//! Pixels are premultiplied by alpha in linear light, then stored as sRGB
//! bytes. A GPU texture in an sRGB format turns them back into linear light
//! as it samples, so filtering and blending are right at transparent edges.
//! Smaller copies (mipmaps) are made at once, so shrunk images are smooth.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use crate::{Error, Rgba};

/// The widest or tallest texture made. Larger images keep their size, and
/// are drawn from a smaller copy.
pub const MAX_TEXTURE: u32 = 8192;

/// One size of an image: premultiplied linear-light pixels stored as sRGB
/// bytes, red, green, blue and alpha, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
  /// Width in pixels.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
  /// Four bytes for each pixel.
  pub pixels: Vec<u8>,
}

struct Inner {
  id: u64,
  width: u32,
  height: u32,
  levels: Vec<Level>,
}

/// A decoded image with its smaller copies. Cheap to clone and safe to
/// send between threads, so it can be decoded in a task (M11).
#[derive(Clone)]
pub struct Bitmap(Arc<Inner>);

impl std::fmt::Debug for Bitmap {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    write!(
      f,
      "Bitmap({} × {}, #{})",
      self.0.width, self.0.height, self.0.id
    )
  }
}

impl PartialEq for Bitmap {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    self.0.id == other.0.id
  }
}

static NEXT: AtomicU64 = AtomicU64::new(1);

/// A [`Bitmap`] that may have been dropped.
#[derive(Clone, Debug)]
pub struct WeakBitmap(std::sync::Weak<Inner>);

impl WeakBitmap {
  /// True while the image is still kept somewhere.
  pub fn is_alive(&self) -> bool {
    self.0.strong_count() > 0
  }
}

impl Bitmap {
  /// Decodes a PNG, JPEG or GIF file's bytes: an animation's first frame.
  pub fn decode(data: &[u8]) -> Result<Bitmap, Error> {
    let rgba = if crate::png::is_png(data) {
      crate::png::decode(data)?
    } else if crate::jpeg::is_jpeg(data) {
      crate::jpeg::decode(data)?
    } else if crate::gif::is_gif(data) {
      crate::decode_rgba(data)?
    } else {
      return Err(Error::NotAnImage);
    };
    Ok(Bitmap::from_rgba(rgba))
  }

  /// Reads and decodes a PNG or JPEG file.
  pub fn open(path: impl AsRef<Path>) -> Result<Bitmap, Error> {
    let data = std::fs::read(path).map_err(|e| Error::Io(e.to_string()))?;
    Bitmap::decode(&data)
  }

  /// An image from straight (not premultiplied) sRGB pixels.
  pub fn from_rgba(image: Rgba) -> Bitmap {
    let Rgba { width, height, pixels } = image;
    assert_eq!(
      pixels.len(),
      width as usize * height as usize * 4,
      "four bytes for each pixel"
    );
    let mut first = Level {
      width,
      height,
      pixels: premultiply(&pixels),
    };
    // Too large for a texture: start from a smaller copy.
    while first.width > MAX_TEXTURE || first.height > MAX_TEXTURE {
      first = half(&first);
    }
    let mut levels = vec![first];
    loop {
      let last = levels.last().expect("one level");
      if last.width == 1 && last.height == 1 {
        break;
      }
      let next = half(last);
      levels.push(next);
    }
    Bitmap(Arc::new(Inner {
      id: NEXT.fetch_add(1, Ordering::Relaxed),
      width,
      height,
      levels,
    }))
  }

  /// A number that only this image has, for caches.
  pub fn id(&self) -> u64 {
    self.0.id
  }

  /// A handle that does not keep the image alive, so a cache can tell when
  /// it is gone.
  pub fn downgrade(&self) -> WeakBitmap {
    WeakBitmap(Arc::downgrade(&self.0))
  }

  /// Width in pixels.
  pub fn width(&self) -> u32 {
    self.0.width
  }

  /// Height in pixels.
  pub fn height(&self) -> u32 {
    self.0.height
  }

  /// The image at each size, largest first, down to one pixel. The first
  /// is the image's own size, unless that is larger than [`MAX_TEXTURE`].
  pub fn levels(&self) -> &[Level] {
    &self.0.levels
  }

  /// Bytes the image takes on the GPU, with its smaller copies.
  pub fn bytes(&self) -> usize {
    self.0.levels.iter().map(|l| l.pixels.len()).sum()
  }

  /// The first level's pixels as straight sRGB again, for tests.
  pub fn to_rgba(&self) -> Rgba {
    let l = &self.0.levels[0];
    Rgba {
      width: l.width,
      height: l.height,
      pixels: unpremultiply(&l.pixels),
    }
  }
}

/// sRGB byte to linear light.
fn to_linear() -> &'static [f32; 256] {
  static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
  TABLE.get_or_init(|| {
    std::array::from_fn(|i| {
      let c = i as f32 / 255.0;
      if c <= 0.04045 {
        c / 12.92
      } else {
        ((c + 0.055) / 1.055).powf(2.4)
      }
    })
  })
}

/// Linear light, in 1/65535ths, to an sRGB byte.
fn to_srgb() -> &'static [u8] {
  static TABLE: OnceLock<Vec<u8>> = OnceLock::new();
  TABLE.get_or_init(|| {
    (0..=65535u32)
      .map(|i| {
        let c = i as f32 / 65535.0;
        let s = if c <= 0.003_130_8 {
          c * 12.92
        } else {
          1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0 + 0.5) as u8
      })
      .collect()
  })
}

fn encode(linear: f32) -> u8 {
  to_srgb()[(linear.clamp(0.0, 1.0) * 65535.0 + 0.5) as usize]
}

fn premultiply(pixels: &[u8]) -> Vec<u8> {
  let lin = to_linear();
  let mut out = Vec::with_capacity(pixels.len());
  for p in pixels.chunks_exact(4) {
    let a = p[3];
    if a == 255 {
      out.extend_from_slice(p);
      continue;
    }
    let af = f32::from(a) / 255.0;
    out.extend([
      encode(lin[usize::from(p[0])] * af),
      encode(lin[usize::from(p[1])] * af),
      encode(lin[usize::from(p[2])] * af),
      a,
    ]);
  }
  out
}

fn unpremultiply(pixels: &[u8]) -> Vec<u8> {
  let lin = to_linear();
  let mut out = Vec::with_capacity(pixels.len());
  for p in pixels.chunks_exact(4) {
    let a = p[3];
    if a == 255 || a == 0 {
      out.extend_from_slice(&[p[0], p[1], p[2], a][..]);
      continue;
    }
    let af = f32::from(a) / 255.0;
    out.extend([
      encode(lin[usize::from(p[0])] / af),
      encode(lin[usize::from(p[1])] / af),
      encode(lin[usize::from(p[2])] / af),
      a,
    ]);
  }
  out
}

/// The next smaller copy: half the size, each pixel the average of those it
/// covers, in linear light. An odd last row or column goes into the pixels
/// beside it.
fn half(level: &Level) -> Level {
  let (w, h) = (level.width as usize, level.height as usize);
  let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
  let lin = to_linear();
  let mut pixels = vec![0u8; nw * nh * 4];
  let span = |n: usize, size: usize, new: usize| {
    let start = (n * 2).min(size - 1);
    let end = if n == new - 1 {
      size
    } else {
      (n * 2 + 2).min(size)
    };
    start..end.max(start + 1)
  };
  for y in 0..nh {
    let ys = span(y, h, nh);
    for x in 0..nw {
      let xs = span(x, w, nw);
      let mut sum = [0.0f32; 4];
      let mut count = 0.0f32;
      for sy in ys.clone() {
        for sx in xs.clone() {
          let p = &level.pixels[(sy * w + sx) * 4..(sy * w + sx) * 4 + 4];
          for c in 0..3 {
            sum[c] += lin[usize::from(p[c])];
          }
          sum[3] += f32::from(p[3]);
          count += 1.0;
        }
      }
      let out = &mut pixels[(y * nw + x) * 4..(y * nw + x) * 4 + 4];
      for c in 0..3 {
        out[c] = encode(sum[c] / count);
      }
      out[3] = (sum[3] / count + 0.5) as u8;
    }
  }
  Level {
    width: nw as u32,
    height: nh as u32,
    pixels,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn solid(
    w: u32,
    h: u32,
    rgba: [u8; 4],
  ) -> Rgba {
    Rgba {
      width: w,
      height: h,
      pixels: rgba.repeat((w * h) as usize),
    }
  }

  #[test]
  fn every_level_down_to_one_pixel() {
    let b = Bitmap::from_rgba(solid(5, 3, [10, 20, 30, 255]));
    let sizes: Vec<(u32, u32)> = b
      .levels()
      .iter()
      .map(|l| (l.width, l.height))
      .collect();
    assert_eq!(sizes, [(5, 3), (2, 1), (1, 1)]);
    for l in b.levels() {
      assert!(
        l.pixels.chunks(4).all(|p| p == [10, 20, 30, 255]),
        "a flat colour stays the same"
      );
    }
  }

  #[test]
  fn halving_averages_in_linear_light() {
    // Black and white side by side average to a mid grey in light, which is
    // 188 in sRGB, not 128.
    let mut image = solid(2, 1, [0, 0, 0, 255]);
    image.pixels[4..8].copy_from_slice(&[255, 255, 255, 255]);
    let b = Bitmap::from_rgba(image);
    assert_eq!(&b.levels()[1].pixels, &[188, 188, 188, 255]);
  }

  #[test]
  fn transparent_pixels_do_not_tint_their_neighbours() {
    // Opaque red beside fully transparent green averages to half-transparent
    // red, with no green.
    let mut image = solid(2, 1, [255, 0, 0, 255]);
    image.pixels[4..8].copy_from_slice(&[0, 255, 0, 0]);
    let b = Bitmap::from_rgba(image);
    let p = &b.levels()[1].pixels;
    assert_eq!(p[1], 0, "no green: {p:?}");
    assert_eq!(p[3], 128);
    assert_eq!(b.to_rgba().pixels, [255, 0, 0, 255, 0, 0, 0, 0]);
  }

  #[test]
  fn premultiplying_round_trips() {
    let image = Rgba {
      width: 1,
      height: 1,
      pixels: vec![200, 100, 50, 128],
    };
    let back = Bitmap::from_rgba(image).to_rgba().pixels;
    for (a, b) in back.iter().zip([200u8, 100, 50, 128]) {
      assert!((i16::from(*a) - i16::from(b)).abs() <= 1, "{back:?}");
    }
  }

  #[test]
  fn images_too_large_for_a_texture_keep_their_size() {
    let b = Bitmap::from_rgba(solid(MAX_TEXTURE * 2 + 2, 2, [1, 2, 3, 255]));
    assert_eq!(b.width(), MAX_TEXTURE * 2 + 2);
    assert!(b.levels()[0].width <= MAX_TEXTURE);
  }
}
