//! PNG decoding: every colour type and bit depth, palettes, `tRNS`
//! transparency and Adam7 interlacing (D0023).
//!
//! Gamma (`gAMA`), colour profiles and other chunks are not read: pixels
//! are taken to be sRGB, as browsers do for most files.

use std::time::Duration;

use crate::{Error, MAX_PIXELS, Rgba};

const SIGNATURE: [u8; 8] = [
  0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a,
];

/// True when `data` starts as a PNG file does.
pub fn is_png(data: &[u8]) -> bool {
  data.starts_with(&SIGNATURE)
}

#[derive(Clone, Copy)]
struct Header {
  width: u32,
  height: u32,
  depth: u8,
  color: u8,
  interlaced: bool,
}

impl Header {
  /// Samples in each pixel.
  fn channels(&self) -> usize {
    match self.color {
      0 | 3 => 1,
      2 => 3,
      4 => 2,
      _ => 4,
    }
  }

  fn bits_per_pixel(&self) -> usize {
    self.channels() * usize::from(self.depth)
  }

  /// The bytes in one filtered row of `width` pixels, without the filter
  /// byte.
  fn row_bytes(
    &self,
    width: u32,
  ) -> usize {
    (width as usize * self.bits_per_pixel()).div_ceil(8)
  }
}

/// Decodes a PNG file into straight (not premultiplied) sRGB pixels. An
/// APNG file gives its default image.
pub fn decode(data: &[u8]) -> Result<Rgba, Error> {
  Ok(read(data)?.image)
}

/// An animation's whole frames, each with its delay, and how many times
/// it plays: `None` for ever.
pub type Frames = (Vec<(Rgba, Duration)>, Option<u32>);

/// A PNG file's image, and an APNG file's frames.
pub struct Decoded {
  /// The default image.
  pub image: Rgba,
  /// An APNG file's frames, each whole, with its delay, and how many times
  /// it plays: `None` for ever.
  pub animation: Option<Frames>,
}

/// One APNG frame's control chunk and its compressed data.
struct FrameData {
  width: u32,
  height: u32,
  x: u32,
  y: u32,
  delay: Duration,
  dispose: u8,
  blend: u8,
  data: Vec<u8>,
}

/// Reads a PNG or APNG file.
pub fn read(data: &[u8]) -> Result<Decoded, Error> {
  if !is_png(data) {
    return Err(Error::NotAnImage);
  }
  let mut at = 8;
  let mut header: Option<Header> = None;
  let mut palette: Vec<[u8; 4]> = Vec::new();
  let mut transparent: Option<[u16; 3]> = None;
  let mut compressed = Vec::new();
  let mut ended = false;
  let mut seen_idat = false;
  // APNG: the frame count and plays, and each frame.
  let mut actl: Option<(u32, u32)> = None;
  let mut frames: Vec<FrameData> = Vec::new();
  let mut idat_is_frame = false;
  while at + 8 <= data.len() {
    let len = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
    let kind: [u8; 4] = data[at + 4..at + 8].try_into().unwrap();
    let body_end = at
      .checked_add(8 + len)
      .filter(|&e| e + 4 <= data.len())
      .ok_or(Error::Truncated)?;
    let body = &data[at + 8..body_end];
    let crc =
      u32::from_be_bytes(data[body_end..body_end + 4].try_into().unwrap());
    if crc32(&data[at + 4..body_end]) != crc {
      return Err(Error::Invalid("a PNG chunk with a bad check"));
    }
    at = body_end + 4;
    if header.is_none() && &kind != b"IHDR" {
      return Err(Error::Invalid("a PNG file that does not start with IHDR"));
    }
    match &kind {
      b"IHDR" => {
        if header.is_some() || body.len() != 13 {
          return Err(Error::Invalid("a bad PNG header"));
        }
        let h = Header {
          width: u32::from_be_bytes(body[0..4].try_into().unwrap()),
          height: u32::from_be_bytes(body[4..8].try_into().unwrap()),
          depth: body[8],
          color: body[9],
          interlaced: match body[12] {
            0 => false,
            1 => true,
            _ => return Err(Error::Invalid("an unknown PNG interlace method")),
          },
        };
        let depth_ok = match h.color {
          0 => matches!(h.depth, 1 | 2 | 4 | 8 | 16),
          3 => matches!(h.depth, 1 | 2 | 4 | 8),
          2 | 4 | 6 => matches!(h.depth, 8 | 16),
          _ => false,
        };
        if !depth_ok || body[10] != 0 || body[11] != 0 {
          return Err(Error::Invalid(
            "a PNG colour type or depth that is not allowed",
          ));
        }
        if h.width == 0 || h.height == 0 {
          return Err(Error::Invalid("a PNG image with no pixels"));
        }
        if u64::from(h.width) * u64::from(h.height) > MAX_PIXELS {
          return Err(Error::TooLarge);
        }
        header = Some(h);
      },
      b"PLTE" => {
        if !body.len().is_multiple_of(3) || body.len() > 768 || body.is_empty()
        {
          return Err(Error::Invalid("a bad PNG palette"));
        }
        palette = body
          .chunks(3)
          .map(|c| [c[0], c[1], c[2], 255])
          .collect();
      },
      b"tRNS" => {
        let h = header.expect("checked above");
        match h.color {
          0 if body.len() >= 2 => {
            let g = u16::from_be_bytes([body[0], body[1]]);
            transparent = Some([g, g, g]);
          },
          2 if body.len() >= 6 => {
            transparent = Some([
              u16::from_be_bytes([body[0], body[1]]),
              u16::from_be_bytes([body[2], body[3]]),
              u16::from_be_bytes([body[4], body[5]]),
            ]);
          },
          3 => {
            for (entry, &a) in palette.iter_mut().zip(body) {
              entry[3] = a;
            }
          },
          _ => {},
        }
      },
      b"IDAT" => {
        // A frame control before the image data makes it the first frame.
        if !seen_idat {
          idat_is_frame = frames.len() == 1;
        }
        if idat_is_frame {
          frames[0].data.extend_from_slice(body);
        }
        seen_idat = true;
        compressed.extend_from_slice(body);
      },
      b"acTL" if body.len() >= 8 => {
        let count = u32::from_be_bytes(body[0..4].try_into().unwrap());
        let plays = u32::from_be_bytes(body[4..8].try_into().unwrap());
        actl = Some((count, plays));
      },
      b"fcTL" if body.len() >= 26 && actl.is_some() => {
        let u =
          |i: usize| u32::from_be_bytes(body[i..i + 4].try_into().unwrap());
        let num = u16::from_be_bytes([body[20], body[21]]);
        let den = u16::from_be_bytes([body[22], body[23]]);
        let den = if den == 0 { 100 } else { den };
        frames.push(FrameData {
          width: u(4),
          height: u(8),
          x: u(12),
          y: u(16),
          delay: Duration::from_secs_f64(f64::from(num) / f64::from(den)),
          dispose: body[24],
          blend: body[25],
          data: Vec::new(),
        });
      },
      b"fdAT" if body.len() >= 4 => {
        if let Some(f) = frames.last_mut() {
          f.data.extend_from_slice(&body[4..]);
        }
      },
      b"IEND" => {
        ended = true;
        break;
      },
      _ => {
        // An unknown chunk that is marked critical cannot be skipped.
        if kind[0] & 0x20 == 0 {
          return Err(Error::Unsupported("a critical PNG chunk"));
        }
      },
    }
  }
  let header = header.ok_or(Error::Truncated)?;
  // A frame control after the default image whose data never came.
  frames.retain(|f| !f.data.is_empty());
  if !seen_idat {
    return Err(Error::Invalid("a PNG file with no image data"));
  }
  if !ended && compressed.is_empty() {
    return Err(Error::Truncated);
  }
  if header.color == 3 && palette.is_empty() {
    return Err(Error::Invalid("a palette PNG with no palette"));
  }

  let image = unpack(&header, &compressed, &palette, transparent)?;
  let animation = match actl {
    Some((_, plays)) if !frames.is_empty() => {
      Some(compose(&header, frames, plays, &palette, transparent)?)
    },
    _ => None,
  };
  Ok(Decoded { image, animation })
}

/// Unpacks a PNG image's compressed data, with `header`'s size, into
/// straight sRGB pixels.
fn unpack(
  header: &Header,
  compressed: &[u8],
  palette: &[[u8; 4]],
  transparent: Option<[u16; 3]>,
) -> Result<Rgba, Error> {
  let header = *header;
  let passes: Vec<(usize, usize, usize, usize)> = if header.interlaced {
    // Each pass: first column, first row, column step, row step.
    vec![
      (0, 0, 8, 8),
      (4, 0, 8, 8),
      (0, 4, 4, 8),
      (2, 0, 4, 4),
      (0, 2, 2, 4),
      (1, 0, 2, 2),
      (0, 1, 1, 2),
    ]
  } else {
    vec![(0, 0, 1, 1)]
  };
  let (w, h) = (header.width as usize, header.height as usize);
  let pass_size = |&(x0, y0, dx, dy): &(usize, usize, usize, usize)| {
    let pw = if w > x0 { (w - x0).div_ceil(dx) } else { 0 };
    let ph = if h > y0 { (h - y0).div_ceil(dy) } else { 0 };
    (pw, ph)
  };
  let expected: usize = passes
    .iter()
    .map(|p| {
      let (pw, ph) = pass_size(p);
      if pw == 0 {
        0
      } else {
        (header.row_bytes(pw as u32) + 1) * ph
      }
    })
    .sum();
  let raw = crate::inflate::zlib(compressed, expected)?;
  if raw.len() < expected {
    return Err(Error::Truncated);
  }

  let mut pixels = vec![0u8; w * h * 4];
  let bpp = header.bits_per_pixel().div_ceil(8).max(1);
  let mut at = 0;
  for pass in &passes {
    let (pw, ph) = pass_size(pass);
    if pw == 0 || ph == 0 {
      continue;
    }
    let row = header.row_bytes(pw as u32);
    let mut previous = vec![0u8; row];
    let mut current = vec![0u8; row];
    let (x0, y0, dx, dy) = *pass;
    for py in 0..ph {
      let filter = raw[at];
      current.copy_from_slice(&raw[at + 1..at + 1 + row]);
      at += row + 1;
      unfilter(filter, &mut current, &previous, bpp)?;
      let y = y0 + py * dy;
      for px in 0..pw {
        let x = x0 + px * dx;
        let rgba = pixel(&header, &current, px, palette, transparent)?;
        pixels[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&rgba);
      }
      std::mem::swap(&mut previous, &mut current);
    }
  }
  Ok(Rgba {
    width: header.width,
    height: header.height,
    pixels,
  })
}

/// The most bytes an animation's frames may take, once decoded.
pub const MAX_ANIMATION_BYTES: u64 = 1 << 30;

/// Puts APNG frames together, each drawn over what the frames before left,
/// as their blend and dispose operations say.
fn compose(
  header: &Header,
  frames: Vec<FrameData>,
  plays: u32,
  palette: &[[u8; 4]],
  transparent: Option<[u16; 3]>,
) -> Result<Frames, Error> {
  let (w, h) = (header.width as usize, header.height as usize);
  if (w * h * 4) as u64 * frames.len() as u64 > MAX_ANIMATION_BYTES {
    return Err(Error::TooLarge);
  }
  let mut canvas = vec![0u8; w * h * 4];
  let mut out = Vec::new();
  for f in frames {
    let (fx, fy, fw, fh) = (
      f.x as usize,
      f.y as usize,
      f.width as usize,
      f.height as usize,
    );
    if fw == 0 || fh == 0 || fx + fw > w || fy + fh > h {
      return Err(Error::Invalid("an APNG frame outside the image"));
    }
    let fh_header = Header {
      width: f.width,
      height: f.height,
      ..*header
    };
    let part = unpack(&fh_header, &f.data, palette, transparent)?;
    let before = (f.dispose == 2).then(|| canvas.clone());
    for y in 0..fh {
      for x in 0..fw {
        let src = &part.pixels[(y * fw + x) * 4..(y * fw + x) * 4 + 4];
        let at = ((fy + y) * w + fx + x) * 4;
        let dst = &mut canvas[at..at + 4];
        if f.blend == 0 || src[3] == 255 {
          dst.copy_from_slice(src);
        } else if src[3] > 0 {
          over(dst, src);
        }
      }
    }
    // A delay too short to see is shown for 100 ms, as browsers do.
    let delay = if f.delay < Duration::from_millis(20) {
      Duration::from_millis(100)
    } else {
      f.delay
    };
    out.push((
      Rgba {
        width: header.width,
        height: header.height,
        pixels: canvas.clone(),
      },
      delay,
    ));
    match f.dispose {
      1 => {
        for y in fy..fy + fh {
          canvas[(y * w + fx) * 4..(y * w + fx + fw) * 4].fill(0);
        }
      },
      2 => {
        if let Some(b) = before {
          canvas = b;
        }
      },
      _ => {},
    }
  }
  Ok((out, (plays != 0).then_some(plays)))
}

/// Draws straight sRGB `src` over `dst`, in sRGB, as browsers blend
/// animation frames.
pub(crate) fn over(
  dst: &mut [u8],
  src: &[u8],
) {
  let sa = f32::from(src[3]) / 255.0;
  let da = f32::from(dst[3]) / 255.0;
  let a = sa + da * (1.0 - sa);
  if a <= 0.0 {
    dst.fill(0);
    return;
  }
  for k in 0..3 {
    let c = (f32::from(src[k]) * sa + f32::from(dst[k]) * da * (1.0 - sa)) / a;
    dst[k] = (c + 0.5) as u8;
  }
  dst[3] = (a * 255.0 + 0.5) as u8;
}

fn unfilter(
  filter: u8,
  row: &mut [u8],
  previous: &[u8],
  bpp: usize,
) -> Result<(), Error> {
  match filter {
    0 => {},
    1 => {
      for i in bpp..row.len() {
        row[i] = row[i].wrapping_add(row[i - bpp]);
      }
    },
    2 => {
      for i in 0..row.len() {
        row[i] = row[i].wrapping_add(previous[i]);
      }
    },
    3 => {
      for i in 0..row.len() {
        let left = if i >= bpp { row[i - bpp] } else { 0 };
        let average = ((u16::from(left) + u16::from(previous[i])) / 2) as u8;
        row[i] = row[i].wrapping_add(average);
      }
    },
    4 => {
      for i in 0..row.len() {
        let a = if i >= bpp { row[i - bpp] } else { 0 };
        let b = previous[i];
        let c = if i >= bpp { previous[i - bpp] } else { 0 };
        row[i] = row[i].wrapping_add(paeth(a, b, c));
      }
    },
    _ => return Err(Error::Invalid("an unknown PNG filter")),
  }
  Ok(())
}

fn paeth(
  a: u8,
  b: u8,
  c: u8,
) -> u8 {
  let p = i16::from(a) + i16::from(b) - i16::from(c);
  let (pa, pb, pc) = (
    (p - i16::from(a)).abs(),
    (p - i16::from(b)).abs(),
    (p - i16::from(c)).abs(),
  );
  if pa <= pb && pa <= pc {
    a
  } else if pb <= pc {
    b
  } else {
    c
  }
}

/// Sample `n` of a row, at the image's bit depth.
fn sample(
  header: &Header,
  row: &[u8],
  n: usize,
) -> u16 {
  match header.depth {
    16 => u16::from_be_bytes([row[n * 2], row[n * 2 + 1]]),
    8 => u16::from(row[n]),
    d => {
      let d = usize::from(d);
      let bit = n * d;
      let byte = row[bit / 8];
      let shift = 8 - d - bit % 8;
      u16::from((byte >> shift) & ((1u8 << d) - 1))
    },
  }
}

/// A sample at the image's depth as 8 bits, rounded.
fn to_8(
  value: u16,
  depth: u8,
) -> u8 {
  match depth {
    16 => ((u32::from(value) * 255 + 32767) / 65535) as u8,
    8 => value as u8,
    d => {
      let max = (1u32 << d) - 1;
      (u32::from(value) * 255 / max) as u8
    },
  }
}

fn pixel(
  header: &Header,
  row: &[u8],
  x: usize,
  palette: &[[u8; 4]],
  transparent: Option<[u16; 3]>,
) -> Result<[u8; 4], Error> {
  let c = header.channels();
  let s = |i: usize| sample(header, row, x * c + i);
  let d = header.depth;
  Ok(match header.color {
    0 => {
      let g = s(0);
      let a = if transparent.is_some_and(|t| t[0] == g) {
        0
      } else {
        255
      };
      let g = to_8(g, d);
      [g, g, g, a]
    },
    2 => {
      let (r, g, b) = (s(0), s(1), s(2));
      let a = if transparent == Some([r, g, b]) {
        0
      } else {
        255
      };
      [to_8(r, d), to_8(g, d), to_8(b, d), a]
    },
    3 => *palette
      .get(usize::from(s(0)))
      .ok_or(Error::Invalid("a PNG pixel outside its palette"))?,
    4 => {
      let g = to_8(s(0), d);
      [g, g, g, to_8(s(1), d)]
    },
    _ => [
      to_8(s(0), d),
      to_8(s(1), d),
      to_8(s(2), d),
      to_8(s(3), d),
    ],
  })
}

/// The CRC-32 that PNG chunks carry.
pub fn crc32(data: &[u8]) -> u32 {
  static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
  let table = TABLE.get_or_init(|| {
    let mut t = [0u32; 256];
    for (n, entry) in t.iter_mut().enumerate() {
      let mut c = n as u32;
      for _ in 0..8 {
        c = if c & 1 != 0 {
          0xedb8_8320 ^ (c >> 1)
        } else {
          c >> 1
        };
      }
      *entry = c;
    }
    t
  });
  let mut c = 0xffff_ffffu32;
  for &b in data {
    c = table[((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8);
  }
  c ^ 0xffff_ffff
}

/// Writes straight sRGB pixels as a PNG file. The data is stored, not
/// compressed, so the file is large: for tests and tools, such as saving
/// what was painted.
pub fn encode(image: &Rgba) -> Vec<u8> {
  let row = image.width as usize * 4;
  let mut raw = Vec::with_capacity((row + 1) * image.height as usize);
  for y in 0..image.height as usize {
    raw.push(0);
    raw.extend_from_slice(&image.pixels[y * row..(y + 1) * row]);
  }
  let mut z = vec![0x78, 0x01];
  let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
  for (i, block) in blocks.iter().enumerate() {
    z.push(u8::from(i + 1 == blocks.len()));
    let len = block.len() as u16;
    z.extend(len.to_le_bytes());
    z.extend((!len).to_le_bytes());
    z.extend_from_slice(block);
  }
  if blocks.is_empty() {
    z.extend([1, 0, 0, 0xff, 0xff]);
  }
  z.extend(crate::inflate::adler32(&raw).to_be_bytes());
  let mut out = SIGNATURE.to_vec();
  let mut header = image.width.to_be_bytes().to_vec();
  header.extend(image.height.to_be_bytes());
  header.extend([8, 6, 0, 0, 0]);
  for (kind, body) in [
    (b"IHDR", &header),
    (b"IDAT", &z),
    (b"IEND", &Vec::new()),
  ] {
    out.extend((body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend(kind);
    out.extend(body);
    let crc = crc32(&out[start..]);
    out.extend(crc.to_be_bytes());
  }
  out
}

/// One frame to write with [`encode_apng`]: its pixels and where they go,
/// how long it shows, and how it is drawn and cleared.
#[derive(Clone, Debug)]
pub struct ApngFrame {
  /// The frame's pixels.
  pub image: Rgba,
  /// Pixels from the canvas's left.
  pub x: u32,
  /// Pixels from the canvas's top.
  pub y: u32,
  /// How long it shows, to the millisecond.
  pub delay: Duration,
  /// After it shows: 0 leaves it, 1 clears its rectangle, 2 puts back
  /// what was there before.
  pub dispose: u8,
  /// 0 replaces what is below; 1 draws over it.
  pub blend: u8,
}

/// Writes an APNG file of `width` × `height` whose default image is the
/// first frame, which must cover the canvas, played `plays` times (0 for
/// ever). The data is stored, not compressed. For tests and tools.
pub fn encode_apng(
  width: u32,
  height: u32,
  frames: &[ApngFrame],
  plays: u32,
) -> Vec<u8> {
  let chunk = |out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]| {
    out.extend((body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend(kind);
    out.extend(body);
    let crc = crc32(&out[start..]);
    out.extend(crc.to_be_bytes());
  };
  let zlib = |image: &Rgba| -> Vec<u8> {
    // The IDAT data of a whole PNG file of this image.
    let file = encode(image);
    let at = file
      .windows(4)
      .position(|w| w == b"IDAT")
      .expect("IDAT");
    let len = u32::from_be_bytes(file[at - 4..at].try_into().unwrap()) as usize;
    file[at + 4..at + 4 + len].to_vec()
  };
  let mut out = SIGNATURE.to_vec();
  let mut header = width.to_be_bytes().to_vec();
  header.extend(height.to_be_bytes());
  header.extend([8, 6, 0, 0, 0]);
  chunk(&mut out, b"IHDR", &header);
  let mut actl = (frames.len() as u32).to_be_bytes().to_vec();
  actl.extend(plays.to_be_bytes());
  chunk(&mut out, b"acTL", &actl);
  let mut sequence = 0u32;
  for (n, f) in frames.iter().enumerate() {
    let mut fctl = sequence.to_be_bytes().to_vec();
    sequence += 1;
    for v in [f.image.width, f.image.height, f.x, f.y] {
      fctl.extend(v.to_be_bytes());
    }
    fctl.extend(((f.delay.as_millis()).min(65535) as u16).to_be_bytes());
    fctl.extend(1000u16.to_be_bytes());
    fctl.extend([f.dispose, f.blend]);
    chunk(&mut out, b"fcTL", &fctl);
    let data = zlib(&f.image);
    if n == 0 {
      chunk(&mut out, b"IDAT", &data);
    } else {
      let mut fdat = sequence.to_be_bytes().to_vec();
      sequence += 1;
      fdat.extend(data);
      chunk(&mut out, b"fdAT", &fdat);
    }
  }
  chunk(&mut out, b"IEND", &[]);
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn crc_matches_a_known_value() {
    assert_eq!(crc32(b"IEND"), 0xae42_6082);
  }

  #[test]
  fn paeth_picks_the_nearest() {
    assert_eq!(paeth(10, 20, 10), 20);
    assert_eq!(paeth(20, 10, 10), 20);
    assert_eq!(paeth(0, 0, 0), 0);
  }

  #[test]
  fn small_depths_are_scaled_to_full_range() {
    assert_eq!(to_8(1, 1), 255);
    assert_eq!(to_8(3, 2), 255);
    assert_eq!(to_8(1, 2), 85);
    assert_eq!(to_8(65535, 16), 255);
    assert_eq!(to_8(0x8080, 16), 128);
  }
}
