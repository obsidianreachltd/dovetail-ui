//! GIF decoding (D0024): GIF87a and GIF89a, global and local palettes,
//! transparency, interlacing, every disposal method, delays and the loop
//! count. Each frame is made whole, drawn over what the frames before it
//! left.

use std::time::Duration;

use crate::png::{Frames, MAX_ANIMATION_BYTES};
use crate::{Error, MAX_PIXELS, Rgba};

/// True when `data` starts as a GIF file does.
pub fn is_gif(data: &[u8]) -> bool {
  data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")
}

struct Reader<'a> {
  data: &'a [u8],
  at: usize,
}

impl Reader<'_> {
  fn byte(&mut self) -> Result<u8, Error> {
    let b = *self.data.get(self.at).ok_or(Error::Truncated)?;
    self.at += 1;
    Ok(b)
  }

  fn u16(&mut self) -> Result<u16, Error> {
    Ok(u16::from(self.byte()?) | u16::from(self.byte()?) << 8)
  }

  fn bytes(
    &mut self,
    n: usize,
  ) -> Result<&[u8], Error> {
    let end = self.at.checked_add(n).ok_or(Error::Truncated)?;
    let s = self
      .data
      .get(self.at..end)
      .ok_or(Error::Truncated)?;
    self.at = end;
    Ok(s)
  }

  fn palette(
    &mut self,
    size: usize,
  ) -> Result<Vec<[u8; 3]>, Error> {
    Ok(
      self
        .bytes(size * 3)?
        .chunks(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect(),
    )
  }

  /// Sub-blocks, joined, until the empty one.
  fn blocks(&mut self) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    loop {
      let n = usize::from(self.byte()?);
      if n == 0 {
        return Ok(out);
      }
      out.extend_from_slice(self.bytes(n)?);
    }
  }
}

/// Decodes every frame of a GIF file. A file cut short gives the frames it
/// has, if it has one.
pub fn decode(data: &[u8]) -> Result<Frames, Error> {
  if !is_gif(data) {
    return Err(Error::NotAnImage);
  }
  let mut r = Reader { data, at: 6 };
  let width = usize::from(r.u16()?);
  let height = usize::from(r.u16()?);
  if width == 0 || height == 0 {
    return Err(Error::Invalid("a GIF image with no pixels"));
  }
  if (width * height) as u64 > MAX_PIXELS {
    return Err(Error::TooLarge);
  }
  let packed = r.byte()?;
  let _background = r.byte()?;
  let _aspect = r.byte()?;
  let global = if packed & 0x80 != 0 {
    r.palette(2 << (packed & 7))?
  } else {
    Vec::new()
  };

  let mut canvas = vec![0u8; width * height * 4];
  let mut frames: Vec<(Rgba, Duration)> = Vec::new();
  // Plays once without a looping extension.
  let mut loops = Some(1);
  // From the graphic control extension, for the next image.
  let mut delay = 0u16;
  let mut disposal = 0u8;
  let mut transparent: Option<u8> = None;
  let mut result = Ok(());
  loop {
    let block = match r.byte() {
      Ok(b) => b,
      Err(e) => {
        result = Err(e);
        break;
      },
    };
    match block {
      0x3b => break,
      0x21 => {
        let step = (|| -> Result<(), Error> {
          let label = r.byte()?;
          let body = r.blocks()?;
          match label {
            0xf9 if body.len() >= 4 => {
              disposal = (body[0] >> 2) & 7;
              delay = u16::from(body[1]) | u16::from(body[2]) << 8;
              transparent = (body[0] & 1 != 0).then_some(body[3]);
            },
            0xff
              if body.starts_with(b"NETSCAPE2.0")
                || body.starts_with(b"ANIMEXTS1.0") =>
            {
              // The application id (11 bytes), then 1 and a count of
              // times to play again: 0 for ever.
              if body.len() >= 14 && body[11] == 1 {
                let n = u32::from(body[12]) | u32::from(body[13]) << 8;
                loops = if n == 0 { None } else { Some(n + 1) };
              }
            },
            _ => {},
          }
          Ok(())
        })();
        if let Err(e) = step {
          result = Err(e);
          break;
        }
      },
      0x2c => {
        let frame = (|| -> Result<(), Error> {
          let left = usize::from(r.u16()?);
          let top = usize::from(r.u16()?);
          let w = usize::from(r.u16()?);
          let h = usize::from(r.u16()?);
          let packed = r.byte()?;
          let local = if packed & 0x80 != 0 {
            Some(r.palette(2 << (packed & 7))?)
          } else {
            None
          };
          let interlaced = packed & 0x40 != 0;
          let min_size = r.byte()?;
          let data = r.blocks()?;
          if (frames.len() as u64 + 1) * (width * height * 4) as u64
            > MAX_ANIMATION_BYTES
          {
            return Err(Error::TooLarge);
          }
          let palette = local.as_ref().unwrap_or(&global);
          let indexes = lzw(&data, min_size, w * h)?;
          let before = (disposal == 3).then(|| canvas.clone());
          // Rows in the order they are stored.
          let rows: Vec<usize> = if interlaced {
            (0..h)
              .step_by(8)
              .chain((4..h).step_by(8))
              .chain((2..h).step_by(4))
              .chain((1..h).step_by(2))
              .collect()
          } else {
            (0..h).collect()
          };
          for (n, &row) in rows.iter().enumerate() {
            let y = top + row;
            if y >= height {
              continue;
            }
            for x in 0..w {
              let Some(&i) = indexes.get(n * w + x) else {
                break;
              };
              if Some(i) == transparent || left + x >= width {
                continue;
              }
              // An index past the palette's end is black, as browsers
              // show it.
              let c = palette
                .get(usize::from(i))
                .copied()
                .unwrap_or([0, 0, 0]);
              let at = (y * width + left + x) * 4;
              canvas[at..at + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
          }
          // A delay too short to see is shown for 100 ms, as browsers do.
          let shown = if delay < 2 {
            100
          } else {
            u64::from(delay) * 10
          };
          frames.push((
            Rgba {
              width: width as u32,
              height: height as u32,
              pixels: canvas.clone(),
            },
            Duration::from_millis(shown),
          ));
          match disposal {
            2 => {
              for y in top..(top + h).min(height) {
                let (a, b) = (left.min(width), (left + w).min(width));
                canvas[(y * width + a) * 4..(y * width + b) * 4].fill(0);
              }
            },
            3 => {
              if let Some(b) = before {
                canvas = b;
              }
            },
            _ => {},
          }
          delay = 0;
          disposal = 0;
          transparent = None;
          Ok(())
        })();
        if let Err(e) = frame {
          result = Err(e);
          break;
        }
      },
      _ => {
        result = Err(Error::Invalid("an unknown GIF block"));
        break;
      },
    }
  }
  match result {
    Err(Error::TooLarge) => Err(Error::TooLarge),
    Err(e) if frames.is_empty() => Err(e),
    _ if frames.is_empty() => Err(Error::Invalid("a GIF file with no image")),
    _ => Ok((frames, loops)),
  }
}

/// Decodes GIF's LZW data into `count` palette indexes, at most. Data that
/// ends early gives what it has.
fn lzw(
  data: &[u8],
  min_size: u8,
  count: usize,
) -> Result<Vec<u8>, Error> {
  if !(1..=11).contains(&min_size) {
    return Err(Error::Invalid("a bad GIF code size"));
  }
  let clear = 1usize << min_size;
  let end = clear + 1;
  // Each code's prefix code, last byte and length.
  let mut prefix = vec![0u16; 4096];
  let mut suffix = vec![0u8; 4096];
  let mut length = vec![0u16; 4096];
  for i in 0..clear {
    suffix[i] = i as u8;
    length[i] = 1;
  }
  let mut next = end + 1;
  let mut size = u32::from(min_size) + 1;
  let mut previous: Option<usize> = None;
  let mut out = Vec::with_capacity(count);
  let (mut buffer, mut bits, mut at) = (0u32, 0u32, 0usize);
  let mut stack = Vec::with_capacity(4096);
  while out.len() < count {
    while bits < size {
      let Some(&b) = data.get(at) else {
        return Ok(out);
      };
      buffer |= u32::from(b) << bits;
      bits += 8;
      at += 1;
    }
    let code = (buffer & ((1 << size) - 1)) as usize;
    buffer >>= size;
    bits -= size;
    if code == clear {
      next = end + 1;
      size = u32::from(min_size) + 1;
      previous = None;
      continue;
    }
    if code == end {
      break;
    }
    let Some(prev) = previous else {
      if code >= clear {
        return Err(Error::Invalid("a GIF code before any data"));
      }
      out.push(code as u8);
      previous = Some(code);
      continue;
    };
    // The code's bytes, or for the code being made, the previous code's
    // bytes and their first byte again.
    let known = code < next;
    if !known && code != next {
      return Err(Error::Invalid("a GIF code out of order"));
    }
    let mut c = if known { code } else { prev };
    stack.clear();
    loop {
      stack.push(suffix[c]);
      if length[c] <= 1 {
        break;
      }
      c = usize::from(prefix[c]);
    }
    let first = *stack.last().expect("one byte");
    out.extend(stack.iter().rev());
    if !known {
      out.push(first);
    }
    if next < 4096 {
      prefix[next] = prev as u16;
      suffix[next] = first;
      length[next] = length[prev] + 1;
      next += 1;
      if next == 1 << size && size < 12 {
        size += 1;
      }
    }
    previous = Some(code);
  }
  out.truncate(count);
  Ok(out)
}

/// One frame to write with [`encode`]: its pixels and where they go on the
/// canvas, how long it shows, and what happens to it after.
#[derive(Clone, Debug)]
pub struct GifFrame {
  /// The frame's pixels; fully transparent ones show what is below.
  pub image: Rgba,
  /// Pixels from the canvas's left.
  pub left: u16,
  /// Pixels from the canvas's top.
  pub top: u16,
  /// How long it shows, to the hundredth of a second.
  pub delay: Duration,
  /// What happens after it shows: 1 leaves it, 2 clears its rectangle, 3
  /// puts back what was there before.
  pub disposal: u8,
  /// Its rows are stored interlaced.
  pub interlaced: bool,
}

/// Writes a GIF file of `width` × `height` with `frames`, played `plays`
/// times (`None` for ever). Each frame has a palette of its own, of up to
/// 255 colours and transparent; its codes are stored without compression.
/// For tests and tools. `None` when a frame has more than 255 colours.
pub fn encode(
  width: u16,
  height: u16,
  frames: &[GifFrame],
  plays: Option<u32>,
) -> Option<Vec<u8>> {
  let mut out = b"GIF89a".to_vec();
  out.extend(width.to_le_bytes());
  out.extend(height.to_le_bytes());
  out.extend([0, 0, 0]);
  if plays != Some(1) {
    let repeats = plays.map_or(0, |n| n.saturating_sub(1).min(65535) as u16);
    out.extend([0x21, 0xff, 11]);
    out.extend(b"NETSCAPE2.0");
    out.extend([3, 1]);
    out.extend(repeats.to_le_bytes());
    out.push(0);
  }
  for f in frames {
    let (w, h) = (f.image.width as usize, f.image.height as usize);
    // The palette, with 255 for transparent.
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut indexes = Vec::with_capacity(w * h);
    for p in f.image.pixels.as_chunks::<4>().0 {
      if p[3] < 128 {
        indexes.push(255u8);
        continue;
      }
      let c = [p[0], p[1], p[2]];
      let i = match palette.iter().position(|&q| q == c) {
        Some(i) => i,
        None => {
          if palette.len() == 255 {
            return None;
          }
          palette.push(c);
          palette.len() - 1
        },
      };
      indexes.push(i as u8);
    }
    let cs = (f.delay.as_millis() / 10).min(65535) as u16;
    out.extend([0x21, 0xf9, 4, (f.disposal & 7) << 2 | 1]);
    out.extend(cs.to_le_bytes());
    out.extend([255, 0]);
    out.push(0x2c);
    out.extend(f.left.to_le_bytes());
    out.extend(f.top.to_le_bytes());
    out.extend((w as u16).to_le_bytes());
    out.extend((h as u16).to_le_bytes());
    out.push(0x80 | if f.interlaced { 0x40 } else { 0 } | 7);
    for i in 0..256 {
      out.extend(palette.get(i).copied().unwrap_or([0, 0, 0]));
    }
    let order: Vec<usize> = if f.interlaced {
      (0..h)
        .step_by(8)
        .chain((4..h).step_by(8))
        .chain((2..h).step_by(4))
        .chain((1..h).step_by(2))
        .collect()
    } else {
      (0..h).collect()
    };
    // 9-bit codes, a clear code before the table would need 10 bits.
    let mut codes = vec![256u32];
    let mut since = 0;
    for &row in &order {
      for &i in &indexes[row * w..(row + 1) * w] {
        if since == 254 {
          codes.push(256);
          since = 0;
        }
        codes.push(u32::from(i));
        since += 1;
      }
    }
    codes.push(257);
    let mut data = Vec::new();
    let (mut acc, mut n) = (0u32, 0);
    for c in codes {
      acc |= c << n;
      n += 9;
      while n >= 8 {
        data.push(acc as u8);
        acc >>= 8;
        n -= 8;
      }
    }
    if n > 0 {
      data.push(acc as u8);
    }
    out.push(8);
    for block in data.chunks(255) {
      out.push(block.len() as u8);
      out.extend(block);
    }
    out.push(0);
  }
  out.push(0x3b);
  Some(out)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_code_for_the_entry_being_made() {
    // Four 1s with a 2-bit minimum: clear, 1, 1, then 6 (1, 1) for the
    // entry just made, then 1 in 4 bits once the table reaches 8, and end.
    let codes = [
      (4u32, 3),
      (1, 3),
      (1, 3),
      (6, 3),
      (1, 4),
      (5, 4),
    ];
    let mut bytes = Vec::new();
    let (mut acc, mut n) = (0u32, 0);
    for (c, size) in codes {
      acc |= c << n;
      n += size;
      while n >= 8 {
        bytes.push(acc as u8);
        acc >>= 8;
        n -= 8;
      }
    }
    bytes.push(acc as u8);
    assert_eq!(lzw(&bytes, 2, 100).unwrap(), [1, 1, 1, 1, 1]);
  }

  #[test]
  fn broken_codes_are_errors_not_panics() {
    let _ = lzw(&[0xff; 20], 2, 100);
    assert!(lzw(&[0], 0, 10).is_err());
  }
}
