//! JPEG decoding (D0023): baseline, extended and progressive files with
//! Huffman coding, restart markers, any chroma subsampling, greyscale,
//! Adobe CMYK and YCCK, and EXIF orientation.
//!
//! Every scan's coefficients are kept until the end, then turned into
//! pixels, so baseline and progressive files share the last steps. A file
//! cut short decodes as far as it goes, with the rest grey.

use crate::{Error, MAX_PIXELS, Rgba};

/// True when `data` starts as a JPEG file does.
pub fn is_jpeg(data: &[u8]) -> bool {
  data.starts_with(&[0xff, 0xd8, 0xff])
}

/// The order coefficients are stored in: zigzag position to row-major index.
const ZIGZAG: [usize; 64] = [
  0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40,
  48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29,
  22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54,
  47, 55, 62, 63,
];

const FAST: u32 = 9;

const MISSING_TABLE: Error =
  Error::Invalid("a JPEG scan that uses a missing table");

#[derive(Clone)]
struct Huffman {
  /// For each 9-bit value: the symbol and its length, or 0 for longer codes.
  fast: Vec<u16>,
  /// The largest code of each length, or -1 when there are none.
  max_code: [i32; 18],
  /// Where each length's symbols start, minus its first code.
  offset: [i32; 17],
  values: Vec<u8>,
}

impl Huffman {
  fn new(
    counts: &[u8; 16],
    values: &[u8],
  ) -> Result<Huffman, Error> {
    let total: usize = counts.iter().map(|&c| usize::from(c)).sum();
    if total > values.len() || total > 256 {
      return Err(Error::Invalid("a bad JPEG Huffman table"));
    }
    let mut fast = vec![0u16; 1 << FAST];
    let mut max_code = [-1i32; 18];
    let mut offset = [0i32; 17];
    let mut code: i32 = 0;
    let mut k: i32 = 0;
    for len in 1..=16usize {
      let count = i32::from(counts[len - 1]);
      offset[len] = k - code;
      if code + count > (1 << len) {
        return Err(Error::Invalid("a JPEG Huffman table with too many codes"));
      }
      if count > 0 {
        for i in 0..count {
          let c = code + i;
          if len as u32 <= FAST {
            let shift = FAST - len as u32;
            let base = (c as u32) << shift;
            let entry = (u16::from(values[(k + i) as usize]) << 4) | len as u16;
            for f in 0..(1u32 << shift) {
              fast[(base + f) as usize] = entry;
            }
          }
        }
        k += count;
        code += count;
        max_code[len] = code - 1;
      }
      if code > (1 << len) {
        return Err(Error::Invalid("a JPEG Huffman table with too many codes"));
      }
      code <<= 1;
    }
    max_code[17] = i32::MAX;
    Ok(Huffman {
      fast,
      max_code,
      offset,
      values: values[..total].to_vec(),
    })
  }
}

/// Reads entropy-coded bits, highest first, skipping stuffed zero bytes.
/// At a marker or the end, it gives zeros.
struct Bits<'a> {
  data: &'a [u8],
  at: usize,
  buffer: u64,
  count: u32,
  /// A marker or the end was reached.
  stopped: bool,
}

impl<'a> Bits<'a> {
  fn new(
    data: &'a [u8],
    at: usize,
  ) -> Self {
    Self {
      data,
      at,
      buffer: 0,
      count: 0,
      stopped: false,
    }
  }

  fn fill(&mut self) {
    while self.count <= 56 {
      let mut byte = 0u8;
      if !self.stopped {
        match self.data.get(self.at) {
          None => self.stopped = true,
          Some(&0xff) => match self.data.get(self.at + 1) {
            Some(0) => {
              byte = 0xff;
              self.at += 2;
            },
            // A run of 0xff before a marker is padding.
            Some(0xff) => {
              self.at += 1;
              continue;
            },
            _ => self.stopped = true,
          },
          Some(&b) => {
            byte = b;
            self.at += 1;
          },
        }
      }
      self.buffer |= u64::from(byte) << (56 - self.count);
      self.count += 8;
    }
  }

  fn bits(
    &mut self,
    n: u32,
  ) -> u32 {
    if n == 0 {
      return 0;
    }
    if self.count < n {
      self.fill();
    }
    let v = (self.buffer >> (64 - n)) as u32;
    self.buffer <<= n;
    self.count -= n;
    v
  }

  fn bit(&mut self) -> bool {
    self.bits(1) == 1
  }

  /// `n` bits as a signed value, as JPEG stores coefficient sizes.
  fn extend(
    &mut self,
    n: u32,
  ) -> i32 {
    if n == 0 {
      return 0;
    }
    if n > 16 {
      return 0;
    }
    let v = self.bits(n) as i32;
    if v < 1 << (n - 1) {
      v - (1 << n) + 1
    } else {
      v
    }
  }

  fn decode(
    &mut self,
    h: &Huffman,
  ) -> Result<u8, Error> {
    if self.count < 16 {
      self.fill();
    }
    let peek = (self.buffer >> (64 - FAST)) as usize;
    let entry = h.fast[peek];
    if entry != 0 {
      let len = u32::from(entry & 15);
      self.buffer <<= len;
      self.count -= len;
      return Ok((entry >> 4) as u8);
    }
    let mut code: i32 = 0;
    for len in 1..=16usize {
      code = (code << 1) | self.bits(1) as i32;
      if code <= h.max_code[len] {
        let index = (h.offset[len] + code) as usize;
        return h
          .values
          .get(index)
          .copied()
          .ok_or(Error::Invalid("a JPEG code outside its table"));
      }
    }
    if self.stopped {
      // Zeros past the end of a cut-short file.
      return Err(Error::Truncated);
    }
    Err(Error::Invalid("a JPEG code that is not in its table"))
  }

  /// Starts again after a restart marker.
  fn restart(&mut self) {
    self.buffer = 0;
    self.count = 0;
    self.stopped = false;
    // Skip to the marker, then past it.
    while self.at + 1 < self.data.len() {
      if self.data[self.at] == 0xff {
        let m = self.data[self.at + 1];
        if (0xd0..=0xd7).contains(&m) {
          self.at += 2;
          return;
        }
        if m != 0 && m != 0xff {
          // Another marker: leave it for the caller.
          return;
        }
      }
      self.at += 1;
    }
  }
}

struct Component {
  id: u8,
  h: usize,
  v: usize,
  quant: usize,
  /// Blocks across and down, filling whole MCUs.
  blocks_w: usize,
  blocks_h: usize,
  /// Coefficients, 64 for each block, row-major inside each block, in 16
  /// bits as libjpeg keeps them.
  coefs: Vec<i16>,
  dc_pred: i32,
  dc_table: usize,
  ac_table: usize,
}

struct Frame {
  width: usize,
  height: usize,
  progressive: bool,
  components: Vec<Component>,
  h_max: usize,
  v_max: usize,
  mcus_x: usize,
  mcus_y: usize,
}

/// Decodes a JPEG file into straight sRGB pixels, turned upright as its
/// EXIF orientation says.
pub fn decode(data: &[u8]) -> Result<Rgba, Error> {
  decode_with(data, false)
}

/// Decodes as [`decode`] does, with subsampled components blended between
/// samples, or, when `nearest`, taken from the nearest sample, as some
/// decoders do. The second is for comparing with their output.
pub fn decode_with(
  data: &[u8],
  nearest: bool,
) -> Result<Rgba, Error> {
  if !is_jpeg(data) {
    return Err(Error::NotAnImage);
  }
  let mut quant = [[0u16; 64]; 4];
  let mut dc_tables: [Option<Huffman>; 4] = Default::default();
  let mut ac_tables: [Option<Huffman>; 4] = Default::default();
  let mut frame: Option<Frame> = None;
  let mut restart_interval = 0usize;
  let mut adobe: Option<u8> = None;
  let mut jfif = false;
  let mut orientation = 1u16;
  let mut at = 2;
  let mut scans = 0;

  loop {
    // Find the next marker.
    while at < data.len() && data[at] != 0xff {
      at += 1;
    }
    while at < data.len() && data[at] == 0xff {
      at += 1;
    }
    let Some(&marker) = data.get(at) else {
      break;
    };
    at += 1;
    match marker {
      0xd8 | 0x01 | 0xd0..=0xd7 => continue,
      0xd9 => break,
      _ => {},
    }
    let len = usize::from(u16::from_be_bytes([
      *data.get(at).ok_or(Error::Truncated)?,
      *data.get(at + 1).ok_or(Error::Truncated)?,
    ]));
    if len < 2 {
      return Err(Error::Invalid("a JPEG segment with a bad length"));
    }
    let body = data
      .get(at + 2..at + len)
      .ok_or(Error::Truncated)?;
    at += len;
    match marker {
      0xdb => {
        let mut b = body;
        while !b.is_empty() {
          let precision = b[0] >> 4;
          let id = usize::from(b[0] & 3);
          let size = if precision == 0 { 64 } else { 128 };
          let values = b.get(1..1 + size).ok_or(Error::Truncated)?;
          for (k, &z) in ZIGZAG.iter().enumerate() {
            quant[id][z] = if precision == 0 {
              u16::from(values[k])
            } else {
              u16::from_be_bytes([values[2 * k], values[2 * k + 1]])
            };
          }
          b = &b[1 + size..];
        }
      },
      0xc4 => {
        let mut b = body;
        while b.len() >= 17 {
          let class = b[0] >> 4;
          let id = usize::from(b[0] & 3);
          let counts: [u8; 16] = b[1..17].try_into().unwrap();
          let total: usize = counts.iter().map(|&c| usize::from(c)).sum();
          let values = b.get(17..17 + total).ok_or(Error::Truncated)?;
          let table = Some(Huffman::new(&counts, values)?);
          if class == 0 {
            dc_tables[id] = table;
          } else {
            ac_tables[id] = table;
          }
          b = &b[17 + total..];
        }
      },
      0xdd => {
        if body.len() < 2 {
          return Err(Error::Truncated);
        }
        restart_interval = usize::from(u16::from_be_bytes([body[0], body[1]]));
      },
      0xe0 => {
        if body.starts_with(b"JFIF\0") {
          jfif = true;
        }
      },
      0xe1 => {
        if let Some(o) = exif_orientation(body) {
          orientation = o;
        }
      },
      0xee => {
        if body.len() >= 12 && body.starts_with(b"Adobe") {
          adobe = Some(body[11]);
        }
      },
      0xc0..=0xc2 => {
        if frame.is_some() {
          return Err(Error::Invalid("a JPEG file with two frames"));
        }
        frame = Some(read_frame(body, marker == 0xc2)?);
      },
      0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf => {
        return Err(Error::Unsupported(
          "lossless, hierarchical or arithmetic-coded JPEG",
        ));
      },
      0xda => {
        let f = frame
          .as_mut()
          .ok_or(Error::Invalid("a JPEG scan before its frame"))?;
        let scan = read_scan(body, f)?;
        scans += 1;
        let end = decode_scan(
          data,
          at,
          f,
          &scan,
          &dc_tables,
          &ac_tables,
          restart_interval,
        );
        match end {
          Ok(next) => at = next,
          // A cut-short file shows what it has.
          Err(Error::Truncated) => break,
          Err(e) => return Err(e),
        }
      },
      _ => {},
    }
  }
  let frame = frame.ok_or(Error::Truncated)?;
  if scans == 0 {
    return Err(Error::Truncated);
  }
  let planes: Vec<Vec<u8>> = frame
    .components
    .iter()
    .map(|c| idct_plane(c, &quant[c.quant]))
    .collect();
  let mut image = colours(&frame, &planes, adobe, jfif, nearest);
  if orientation != 1 {
    image = orient(image, orientation);
  }
  Ok(image)
}

fn read_frame(
  body: &[u8],
  progressive: bool,
) -> Result<Frame, Error> {
  if body.len() < 6 {
    return Err(Error::Truncated);
  }
  if body[0] != 8 {
    return Err(Error::Unsupported("JPEG samples that are not 8 bits"));
  }
  let height = usize::from(u16::from_be_bytes([body[1], body[2]]));
  let width = usize::from(u16::from_be_bytes([body[3], body[4]]));
  let count = usize::from(body[5]);
  if width == 0 || height == 0 {
    return Err(Error::Unsupported("a JPEG file whose height comes later"));
  }
  if (width * height) as u64 > MAX_PIXELS {
    return Err(Error::TooLarge);
  }
  if !matches!(count, 1 | 3 | 4) || body.len() < 6 + 3 * count {
    return Err(Error::Invalid(
      "a JPEG frame with a bad number of components",
    ));
  }
  let mut components = Vec::new();
  for i in 0..count {
    let c = &body[6 + 3 * i..9 + 3 * i];
    let (mut h, mut v) = (usize::from(c[1] >> 4), usize::from(c[1] & 15));
    if !(1..=4).contains(&h) || !(1..=4).contains(&v) {
      return Err(Error::Invalid("a bad JPEG sampling factor"));
    }
    // One component is never subsampled: each MCU is one block.
    if count == 1 {
      (h, v) = (1, 1);
    }
    components.push(Component {
      id: c[0],
      h,
      v,
      quant: usize::from(c[2] & 3),
      blocks_w: 0,
      blocks_h: 0,
      coefs: Vec::new(),
      dc_pred: 0,
      dc_table: 0,
      ac_table: 0,
    });
  }
  let h_max = components.iter().map(|c| c.h).max().unwrap_or(1);
  let v_max = components.iter().map(|c| c.v).max().unwrap_or(1);
  let mcus_x = width.div_ceil(8 * h_max);
  let mcus_y = height.div_ceil(8 * v_max);
  for c in &mut components {
    c.blocks_w = mcus_x * c.h;
    c.blocks_h = mcus_y * c.v;
    c.coefs = vec![0; c.blocks_w * c.blocks_h * 64];
  }
  Ok(Frame {
    width,
    height,
    progressive,
    components,
    h_max,
    v_max,
    mcus_x,
    mcus_y,
  })
}

struct Scan {
  /// Indexes into the frame's components.
  components: Vec<usize>,
  start: usize,
  end: usize,
  high: u32,
  low: u32,
}

fn read_scan(
  body: &[u8],
  frame: &mut Frame,
) -> Result<Scan, Error> {
  let count = usize::from(*body.first().ok_or(Error::Truncated)?);
  if count == 0 || count > 4 || body.len() < 4 + 2 * count {
    return Err(Error::Invalid("a bad JPEG scan header"));
  }
  let mut components = Vec::new();
  for i in 0..count {
    let id = body[1 + 2 * i];
    let tables = body[2 + 2 * i];
    let index = frame
      .components
      .iter()
      .position(|c| c.id == id)
      .ok_or(Error::Invalid(
        "a JPEG scan of a component that is not there",
      ))?;
    frame.components[index].dc_table = usize::from(tables >> 4) & 3;
    frame.components[index].ac_table = usize::from(tables & 15) & 3;
    components.push(index);
  }
  let rest = &body[1 + 2 * count..];
  let (start, end) = (usize::from(rest[0]), usize::from(rest[1]));
  let (high, low) = (u32::from(rest[2] >> 4), u32::from(rest[2] & 15));
  if frame.progressive {
    if start > end || end > 63 || (start == 0 && end != 0) || low > 13 {
      return Err(Error::Invalid("a bad progressive JPEG scan"));
    }
    if start > 0 && count != 1 {
      return Err(Error::Invalid(
        "a progressive AC scan of several components",
      ));
    }
  }
  let (start, end) = if frame.progressive {
    (start, end)
  } else {
    (0, 63)
  };
  Ok(Scan { components, start, end, high, low })
}

/// Decodes one scan's entropy-coded data from `at`, and returns where the
/// next marker is.
#[allow(clippy::too_many_arguments)]
fn decode_scan(
  data: &[u8],
  at: usize,
  frame: &mut Frame,
  scan: &Scan,
  dc_tables: &[Option<Huffman>; 4],
  ac_tables: &[Option<Huffman>; 4],
  restart_interval: usize,
) -> Result<usize, Error> {
  let mut bits = Bits::new(data, at);
  for &c in &scan.components {
    frame.components[c].dc_pred = 0;
  }
  let table =
    |tables: &'_ [Option<Huffman>; 4], i: usize| -> Result<Huffman, Error> {
      tables[i]
        .clone()
        .ok_or(Error::Invalid("a JPEG scan that uses a missing table"))
    };
  // Tables for each component of the scan.
  let mut dcs = Vec::new();
  let mut acs = Vec::new();
  for &c in &scan.components {
    let comp = &frame.components[c];
    let needs_dc = scan.start == 0 && (scan.high == 0 || !frame.progressive);
    dcs.push(if needs_dc {
      Some(table(dc_tables, comp.dc_table)?)
    } else {
      None
    });
    acs.push(if scan.end > 0 {
      Some(table(ac_tables, comp.ac_table)?)
    } else {
      None
    });
  }

  let progressive = frame.progressive;
  let mut eob = 0u32;
  let mut done = 0usize;
  let mut restart = |bits: &mut Bits<'_>, frame: &mut Frame, eob: &mut u32| {
    done += 1;
    if restart_interval > 0 && done.is_multiple_of(restart_interval) {
      bits.restart();
      for &c in &scan.components {
        frame.components[c].dc_pred = 0;
      }
      *eob = 0;
    }
  };

  if scan.components.len() == 1 {
    // One component: its blocks in order, only those inside the image.
    let ci = scan.components[0];
    let (w, h) = {
      let c = &frame.components[ci];
      (
        (frame.width * c.h)
          .div_ceil(frame.h_max)
          .div_ceil(8),
        (frame.height * c.v)
          .div_ceil(frame.v_max)
          .div_ceil(8),
      )
    };
    for by in 0..h {
      for bx in 0..w {
        let comp = &mut frame.components[ci];
        let block = by * comp.blocks_w + bx;
        decode_block(
          &mut bits,
          &mut comp.coefs[block * 64..block * 64 + 64],
          &mut comp.dc_pred,
          dcs[0].as_ref(),
          acs[0].as_ref(),
          scan,
          progressive,
          &mut eob,
        )?;
        restart(&mut bits, frame, &mut eob);
      }
    }
  } else {
    for my in 0..frame.mcus_y {
      for mx in 0..frame.mcus_x {
        for (si, &ci) in scan.components.iter().enumerate() {
          let comp = &mut frame.components[ci];
          for y in 0..comp.v {
            for x in 0..comp.h {
              let block = (my * comp.v + y) * comp.blocks_w + mx * comp.h + x;
              decode_block(
                &mut bits,
                &mut comp.coefs[block * 64..block * 64 + 64],
                &mut comp.dc_pred,
                dcs[si].as_ref(),
                acs[si].as_ref(),
                scan,
                progressive,
                &mut eob,
              )?;
            }
          }
        }
        restart(&mut bits, frame, &mut eob);
      }
    }
  }
  // The next marker, after any data left.
  let mut next = bits.at;
  while next + 1 < data.len() {
    if data[next] == 0xff
      && data[next + 1] != 0
      && !(0xd0..=0xd7).contains(&data[next + 1])
    {
      break;
    }
    next += 1;
  }
  Ok(next)
}

/// A coefficient in 16 bits; damaged files can ask for more.
fn short(v: i32) -> i16 {
  v.clamp(-32768, 32767) as i16
}

/// Decodes one block of a scan into `coefs`.
#[allow(clippy::too_many_arguments)]
fn decode_block(
  bits: &mut Bits<'_>,
  coefs: &mut [i16],
  dc_pred: &mut i32,
  dc: Option<&Huffman>,
  ac: Option<&Huffman>,
  scan: &Scan,
  progressive: bool,
  eob_run: &mut u32,
) -> Result<(), Error> {
  if !progressive {
    let t = bits.decode(dc.ok_or(MISSING_TABLE)?)?;
    let diff = bits.extend(u32::from(t));
    *dc_pred = (*dc_pred + diff).clamp(-(1 << 16), 1 << 16);
    coefs[0] = short(*dc_pred);
    let ac = ac.ok_or(MISSING_TABLE)?;
    let mut k = 1;
    while k < 64 {
      let rs = bits.decode(ac)?;
      let (r, s) = (usize::from(rs >> 4), u32::from(rs & 15));
      if s == 0 {
        if r == 15 {
          k += 16;
          continue;
        }
        break;
      }
      k += r;
      if k > 63 {
        break;
      }
      coefs[ZIGZAG[k]] = short(bits.extend(s));
      k += 1;
    }
    return Ok(());
  }
  if scan.start == 0 {
    if scan.high == 0 {
      let t = bits.decode(dc.ok_or(MISSING_TABLE)?)?;
      *dc_pred =
        (*dc_pred + bits.extend(u32::from(t))).clamp(-(1 << 16), 1 << 16);
      coefs[0] = short(*dc_pred << scan.low);
    } else if bits.bit() {
      coefs[0] |= short(1 << scan.low);
    }
    return Ok(());
  }
  let ac = ac.ok_or(MISSING_TABLE)?;
  if scan.high == 0 {
    // The first pass over these coefficients.
    if *eob_run > 0 {
      *eob_run -= 1;
      return Ok(());
    }
    let mut k = scan.start;
    while k <= scan.end {
      let rs = bits.decode(ac)?;
      let (r, s) = (u32::from(rs >> 4), u32::from(rs & 15));
      if s == 0 {
        if r < 15 {
          *eob_run = (1 << r) - 1;
          if r > 0 {
            *eob_run += bits.bits(r);
          }
          break;
        }
        k += 16;
        continue;
      }
      k += r as usize;
      if k > 63 {
        break;
      }
      coefs[ZIGZAG[k]] = short(bits.extend(s) << scan.low);
      k += 1;
    }
    return Ok(());
  }
  // A refining pass: one more bit for coefficients already seen, and new
  // ones of size one.
  let p1 = 1i32 << scan.low;
  let m1 = -1i32 << scan.low;
  let mut k = scan.start;
  let refine = |bits: &mut Bits<'_>, c: &mut i16| {
    let v = i32::from(*c);
    if bits.bit() && v & p1 == 0 {
      *c = short(v + if v >= 0 { p1 } else { m1 });
    }
  };
  if *eob_run == 0 {
    while k <= scan.end {
      let rs = bits.decode(ac)?;
      let (mut r, s) = (i32::from(rs >> 4), rs & 15);
      let mut value = 0;
      if s == 0 {
        if r < 15 {
          *eob_run = 1 << r;
          if r > 0 {
            *eob_run += bits.bits(r as u32);
          }
          break;
        }
      } else {
        value = if bits.bit() { p1 } else { m1 };
      }
      while k <= scan.end {
        let z = ZIGZAG[k];
        if coefs[z] != 0 {
          refine(bits, &mut coefs[z]);
        } else {
          if r == 0 {
            if value != 0 {
              coefs[z] = short(value);
            }
            k += 1;
            break;
          }
          r -= 1;
        }
        k += 1;
      }
    }
  }
  if *eob_run > 0 {
    while k <= scan.end {
      let z = ZIGZAG[k];
      if coefs[z] != 0 {
        refine(bits, &mut coefs[z]);
      }
      k += 1;
    }
    *eob_run -= 1;
  }
  Ok(())
}

/// Turns a component's coefficients into pixels: one byte for each, in a
/// plane of whole blocks.
fn idct_plane(
  comp: &Component,
  quant: &[u16; 64],
) -> Vec<u8> {
  let stride = comp.blocks_w * 8;
  let mut plane = vec![0u8; stride * comp.blocks_h * 8];
  let mut block = [0i32; 64];
  for by in 0..comp.blocks_h {
    for bx in 0..comp.blocks_w {
      let at = (by * comp.blocks_w + bx) * 64;
      for i in 0..64 {
        // Damaged files can hold any value; real ones stay far inside this,
        // and the transform cannot overflow with it.
        block[i] = (i32::from(comp.coefs[at + i]) * i32::from(quant[i]))
          .clamp(-4096, 4095);
      }
      let out = &mut plane[by * 8 * stride + bx * 8..];
      idct(&block, out, stride);
    }
  }
  plane
}

/// `x` as 12-bit fixed point.
const fn fixed(x: f64) -> i32 {
  (x * 4096.0 + 0.5) as i32
}

/// One 8-point inverse DCT, in the even and odd parts of Loeffler, Ligtenberg
/// and Moschytz's method. Returns the four sums and four differences to pair.
#[inline]
fn idct_1d(s: [i32; 8]) -> ([i32; 4], [i32; 4]) {
  let p1 = (s[2] + s[6]) * fixed(0.541_196_1);
  let t2 = p1 + s[6] * fixed(-1.847_759_065);
  let t3 = p1 + s[2] * fixed(0.765_366_865);
  let t0 = (s[0] + s[4]) * 4096;
  let t1 = (s[0] - s[4]) * 4096;
  let x = [t0 + t3, t1 + t2, t1 - t2, t0 - t3];
  let (mut o0, mut o1, mut o2, mut o3) = (s[7], s[5], s[3], s[1]);
  let p3 = o0 + o2;
  let p4 = o1 + o3;
  let p1 = o0 + o3;
  let p2 = o1 + o2;
  let p5 = (p3 + p4) * fixed(1.175_875_602);
  o0 *= fixed(0.298_631_336);
  o1 *= fixed(2.053_119_869);
  o2 *= fixed(3.072_711_026);
  o3 *= fixed(1.501_321_110);
  let p1 = p5 + p1 * fixed(-0.899_976_223);
  let p2 = p5 + p2 * fixed(-2.562_915_447);
  let p3 = p3 * fixed(-1.961_570_560);
  let p4 = p4 * fixed(-0.390_180_644);
  o3 += p1 + p4;
  o2 += p2 + p3;
  o1 += p2 + p4;
  o0 += p1 + p3;
  (x, [o3, o2, o1, o0])
}

fn idct(
  input: &[i32; 64],
  out: &mut [u8],
  stride: usize,
) {
  let mut v = [0i32; 64];
  for col in 0..8 {
    let s: [i32; 8] = std::array::from_fn(|r| input[r * 8 + col]);
    if s[1..].iter().all(|&c| c == 0) {
      for r in 0..8 {
        v[r * 8 + col] = s[0] * 4;
      }
      continue;
    }
    let (x, t) = idct_1d(s);
    // Down from 12 bits of fraction to 2.
    for i in 0..4 {
      v[i * 8 + col] = (x[i] + 512 + t[i]) >> 10;
      v[(7 - i) * 8 + col] = (x[i] + 512 - t[i]) >> 10;
    }
  }
  for row in 0..8 {
    let s: [i32; 8] = std::array::from_fn(|c| v[row * 8 + c]);
    let (x, t) = idct_1d(s);
    // 12 bits from the constants, 2 from the first pass and 3 from the two
    // scalings by √8: 17 in all, rounded, with 128 added.
    let o = &mut out[row * stride..row * stride + 8];
    for i in 0..4 {
      let base = x[i] + 65536 + (128 << 17);
      o[i] = ((base + t[i]) >> 17).clamp(0, 255) as u8;
      o[7 - i] = ((base - t[i]) >> 17).clamp(0, 255) as u8;
    }
  }
}

/// A component's sample for the pixel at `x`, `y`, blended between the
/// nearest samples when the component is subsampled.
struct Sampler<'a> {
  plane: &'a [u8],
  stride: usize,
  /// The samples there are across and down.
  width: usize,
  height: usize,
  /// Image pixels for each sample.
  sx: f32,
  sy: f32,
  full: bool,
  nearest: bool,
}

impl Sampler<'_> {
  #[inline]
  fn at(
    &self,
    x: usize,
    y: usize,
  ) -> f32 {
    if self.full {
      return f32::from(self.plane[y * self.stride + x]);
    }
    if self.nearest {
      let (sx, sy) =
        ((x as f32 / self.sx) as usize, (y as f32 / self.sy) as usize);
      return f32::from(self.plane[sy * self.stride + sx]);
    }
    // Sample centres sit in the middle of the pixels they cover.
    let fx = ((x as f32 + 0.5) / self.sx - 0.5).max(0.0);
    let fy = ((y as f32 + 0.5) / self.sy - 0.5).max(0.0);
    let (x0, y0) = (fx as usize, fy as usize);
    let (x1, y1) =
      ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
    let (x0, y0) = (x0.min(self.width - 1), y0.min(self.height - 1));
    let (ax, ay) = (fx - fx.floor(), fy - fy.floor());
    let p = |x, y| f32::from(self.plane[y * self.stride + x]);
    let top = p(x0, y0) + (p(x1, y0) - p(x0, y0)) * ax;
    let bottom = p(x0, y1) + (p(x1, y1) - p(x0, y1)) * ax;
    top + (bottom - top) * ay
  }
}

fn colours(
  frame: &Frame,
  planes: &[Vec<u8>],
  adobe: Option<u8>,
  jfif: bool,
  nearest: bool,
) -> Rgba {
  let (w, h) = (frame.width, frame.height);
  let samplers: Vec<Sampler<'_>> = frame
    .components
    .iter()
    .zip(planes)
    .map(|(c, plane)| Sampler {
      plane,
      stride: c.blocks_w * 8,
      width: (w * c.h).div_ceil(frame.h_max).max(1),
      height: (h * c.v).div_ceil(frame.v_max).max(1),
      sx: (frame.h_max / c.h) as f32,
      sy: (frame.v_max / c.v) as f32,
      full: c.h == frame.h_max && c.v == frame.v_max,
      nearest,
    })
    .collect();
  let ids: Vec<u8> = frame.components.iter().map(|c| c.id).collect();
  // Three components are RGB when Adobe says so, or when named R, G, B
  // with no JFIF marker; otherwise YCbCr.
  let rgb = match adobe {
    Some(t) => t == 0,
    None => !jfif && ids == [b'R', b'G', b'B'],
  };
  let ycck = adobe == Some(2);
  let mut pixels = vec![0u8; w * h * 4];
  let ycc = |y: f32, cb: f32, cr: f32| -> [f32; 3] {
    let (cb, cr) = (cb - 128.0, cr - 128.0);
    [
      y + 1.402 * cr,
      y - 0.344_136 * cb - 0.714_136 * cr,
      y + 1.772 * cb,
    ]
  };
  let byte = |v: f32| (v + 0.5).clamp(0.0, 255.0) as u8;
  for y in 0..h {
    for x in 0..w {
      let s: [f32; 4] =
        std::array::from_fn(|i| samplers.get(i).map_or(0.0, |p| p.at(x, y)));
      let out = &mut pixels[(y * w + x) * 4..(y * w + x) * 4 + 4];
      let rgb_out = match samplers.len() {
        1 => [s[0]; 3],
        3 if rgb => [s[0], s[1], s[2]],
        3 => ycc(s[0], s[1], s[2]),
        _ => {
          // Adobe stores CMYK inverted: 255 is no ink.
          let cmy = if ycck {
            ycc(s[0], s[1], s[2])
          } else {
            [s[0], s[1], s[2]]
          };
          let k = s[3] / 255.0;
          [cmy[0] * k, cmy[1] * k, cmy[2] * k]
        },
      };
      out[0] = byte(rgb_out[0]);
      out[1] = byte(rgb_out[1]);
      out[2] = byte(rgb_out[2]);
      out[3] = 255;
    }
  }
  Rgba {
    width: w as u32,
    height: h as u32,
    pixels,
  }
}

/// The orientation in an APP1 EXIF segment: 1 is upright.
fn exif_orientation(body: &[u8]) -> Option<u16> {
  let tiff = body.strip_prefix(b"Exif\0\0")?;
  let little = match tiff.get(0..2)? {
    b"II" => true,
    b"MM" => false,
    _ => return None,
  };
  let u16_at = |at: usize| -> Option<u16> {
    let b = tiff.get(at..at + 2)?;
    Some(if little {
      u16::from_le_bytes([b[0], b[1]])
    } else {
      u16::from_be_bytes([b[0], b[1]])
    })
  };
  let u32_at = |at: usize| -> Option<u32> {
    let b = tiff.get(at..at + 4)?;
    let b = [b[0], b[1], b[2], b[3]];
    Some(if little {
      u32::from_le_bytes(b)
    } else {
      u32::from_be_bytes(b)
    })
  };
  let ifd = u32_at(4)? as usize;
  let count = usize::from(u16_at(ifd)?);
  for i in 0..count.min(512) {
    let entry = ifd + 2 + i * 12;
    if u16_at(entry)? == 0x0112 {
      let o = u16_at(entry + 8)?;
      return (1..=8).contains(&o).then_some(o);
    }
  }
  None
}

/// Turns an image as EXIF orientation `o` (2 to 8) says, so it shows
/// upright.
fn orient(
  image: Rgba,
  o: u16,
) -> Rgba {
  let (w, h) = (image.width as usize, image.height as usize);
  let swap = o >= 5;
  let (nw, nh) = if swap { (h, w) } else { (w, h) };
  let mut pixels = vec![0u8; w * h * 4];
  for y in 0..nh {
    for x in 0..nw {
      // Where the shown pixel comes from in the stored image.
      let (sx, sy) = match o {
        2 => (w - 1 - x, y),
        3 => (w - 1 - x, h - 1 - y),
        4 => (x, h - 1 - y),
        5 => (y, x),
        6 => (y, h - 1 - x),
        7 => (w - 1 - y, h - 1 - x),
        8 => (w - 1 - y, x),
        _ => (x, y),
      };
      let from = (sy * w + sx) * 4;
      let to = (y * nw + x) * 4;
      pixels[to..to + 4].copy_from_slice(&image.pixels[from..from + 4]);
    }
  }
  Rgba {
    width: nw as u32,
    height: nh as u32,
    pixels,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_flat_block_is_its_dc_value() {
    let mut block = [0i32; 64];
    // A DC of 8 × 40 is 40 above the middle grey.
    block[0] = 320;
    let mut out = [0u8; 64];
    idct(&block, &mut out, 8);
    assert!(out.iter().all(|&v| v == 168), "{out:?}");
  }

  #[test]
  fn the_idct_matches_the_definition() {
    let mut block = [0i32; 64];
    block[1] = 100;
    block[9] = -50;
    block[17] = 30;
    let mut out = [0u8; 64];
    idct(&block, &mut out, 8);
    for y in 0..8 {
      for x in 0..8 {
        let mut sum = 0.0f64;
        for v in 0..8 {
          for u in 0..8 {
            let cu = if u == 0 { 1.0 / 2f64.sqrt() } else { 1.0 };
            let cv = if v == 0 { 1.0 / 2f64.sqrt() } else { 1.0 };
            let c = f64::from(block[v * 8 + u]);
            sum += cu
              * cv
              * c
              * (((2 * x + 1) as f64 * u as f64 * std::f64::consts::PI) / 16.0)
                .cos()
              * (((2 * y + 1) as f64 * v as f64 * std::f64::consts::PI) / 16.0)
                .cos();
          }
        }
        let want = (sum / 4.0 + 128.0).round().clamp(0.0, 255.0);
        let got = f64::from(out[y * 8 + x]);
        assert!((got - want).abs() <= 1.0, "({x}, {y}): {got} not {want}");
      }
    }
  }

  #[test]
  fn huffman_codes_are_canonical() {
    // Two codes of length 2 (00, 01) and one of length 3 (100).
    let mut counts = [0u8; 16];
    counts[1] = 2;
    counts[2] = 1;
    let h = Huffman::new(&counts, &[5, 6, 7]).unwrap();
    let data = [0b0001_1000, 0x00];
    let mut bits = Bits::new(&data, 0);
    assert_eq!(bits.decode(&h).unwrap(), 5);
    assert_eq!(bits.decode(&h).unwrap(), 6);
    assert_eq!(bits.decode(&h).unwrap(), 7);
  }

  #[test]
  fn stuffed_bytes_and_markers() {
    let data = [0xff, 0x00, 0x12, 0xff, 0xd9];
    let mut bits = Bits::new(&data, 0);
    assert_eq!(bits.bits(8), 0xff);
    assert_eq!(bits.bits(8), 0x12);
    assert_eq!(bits.bits(8), 0, "zeros at a marker");
    assert!(bits.stopped);
  }

  #[test]
  fn orientations_turn_the_picture() {
    // 2 × 1: red then green.
    let image = Rgba {
      width: 2,
      height: 1,
      pixels: vec![255, 0, 0, 255, 0, 255, 0, 255],
    };
    let turned = orient(image.clone(), 6);
    assert_eq!((turned.width, turned.height), (1, 2));
    assert_eq!(&turned.pixels[..4], &[255, 0, 0, 255], "red on top");
    let mirrored = orient(image, 2);
    assert_eq!(&mirrored.pixels[..4], &[0, 255, 0, 255]);
  }
}
