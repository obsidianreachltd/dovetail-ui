//! Inflate (RFC 1951) and the zlib wrapper around it (RFC 1950), for PNG.
//!
//! Huffman codes are read with a table for the first 10 bits; longer codes,
//! which are rare, are read one bit at a time after that.

use crate::Error;

/// Reads bits from the lowest bit of each byte up, as deflate stores them.
struct Bits<'a> {
  data: &'a [u8],
  at: usize,
  buffer: u64,
  count: u32,
}

impl<'a> Bits<'a> {
  fn new(data: &'a [u8]) -> Self {
    Self { data, at: 0, buffer: 0, count: 0 }
  }

  /// Makes sure at least 32 bits are buffered, if the data has them.
  fn fill(&mut self) {
    while self.count <= 56 {
      let Some(&byte) = self.data.get(self.at) else {
        return;
      };
      self.buffer |= u64::from(byte) << self.count;
      self.at += 1;
      self.count += 8;
    }
  }

  fn need(
    &mut self,
    n: u32,
  ) -> Result<(), Error> {
    if self.count < n {
      self.fill();
      if self.count < n {
        return Err(Error::Truncated);
      }
    }
    Ok(())
  }

  fn take(
    &mut self,
    n: u32,
  ) -> Result<u32, Error> {
    if n == 0 {
      return Ok(0);
    }
    self.need(n)?;
    let v = (self.buffer & ((1u64 << n) - 1)) as u32;
    self.buffer >>= n;
    self.count -= n;
    Ok(v)
  }

  /// Drops the bits left in the current byte.
  fn align(&mut self) {
    let drop = self.count % 8;
    self.buffer >>= drop;
    self.count -= drop;
  }

  /// The bytes left after the buffered bits, once aligned.
  fn bytes(
    &mut self,
    n: usize,
    out: &mut Vec<u8>,
  ) -> Result<(), Error> {
    // Whole bytes still in the buffer come first.
    let mut left = n;
    while left > 0 && self.count >= 8 {
      out.push(self.buffer as u8);
      self.buffer >>= 8;
      self.count -= 8;
      left -= 1;
    }
    let end = self
      .at
      .checked_add(left)
      .ok_or(Error::Truncated)?;
    let slice = self
      .data
      .get(self.at..end)
      .ok_or(Error::Truncated)?;
    out.extend_from_slice(slice);
    self.at = end;
    Ok(())
  }
}

const FAST: u32 = 10;

/// A canonical Huffman code.
struct Huffman {
  /// For each 10-bit value: the symbol and its length, or 0 when the code is
  /// longer.
  fast: Vec<u16>,
  /// How many codes have each length, 0 to 15.
  counts: [u16; 16],
  /// The symbols, ordered by code.
  symbols: Vec<u16>,
}

impl Huffman {
  fn new(lengths: &[u8]) -> Result<Huffman, Error> {
    let mut counts = [0u16; 16];
    for &l in lengths {
      counts[usize::from(l)] += 1;
    }
    counts[0] = 0;
    // A code must not have more codes of a length than fit.
    let mut left: i32 = 1;
    for &count in &counts[1..] {
      left = left * 2 - i32::from(count);
      if left < 0 {
        return Err(Error::Invalid("a Huffman code with too many codes"));
      }
    }
    let mut offsets = [0u16; 16];
    for len in 1..15 {
      offsets[len + 1] = offsets[len] + counts[len];
    }
    let mut symbols = vec![0u16; lengths.len()];
    for (symbol, &l) in lengths.iter().enumerate() {
      if l != 0 {
        symbols[usize::from(offsets[usize::from(l)])] = symbol as u16;
        offsets[usize::from(l)] += 1;
      }
    }
    // The fast table: each code's bits reversed, as they are read.
    let mut fast = vec![0u16; 1 << FAST];
    let mut code: u32 = 0;
    let mut index = 0usize;
    for len in 1..=15u32 {
      for _ in 0..counts[len as usize] {
        if len <= FAST {
          let reversed = reverse(code, len);
          let entry = (symbols[index] << 4) | len as u16;
          let mut fill = reversed;
          while fill < (1 << FAST) {
            fast[fill as usize] = entry;
            fill += 1 << len;
          }
        }
        code += 1;
        index += 1;
      }
      code <<= 1;
    }
    Ok(Huffman { fast, counts, symbols })
  }

  fn decode(
    &self,
    bits: &mut Bits<'_>,
  ) -> Result<u16, Error> {
    if bits.count < 15 {
      bits.fill();
    }
    let entry = self.fast[(bits.buffer & ((1 << FAST) - 1)) as usize];
    let len = u32::from(entry & 15);
    if len != 0 {
      if len > bits.count {
        return Err(Error::Truncated);
      }
      bits.buffer >>= len;
      bits.count -= len;
      return Ok(entry >> 4);
    }
    // A long code, or none: one bit at a time, as in zlib's `puff`.
    let mut code: i32 = 0;
    let mut first: i32 = 0;
    let mut index: i32 = 0;
    for len in 1..16 {
      code |= bits.take(1)? as i32;
      let count = i32::from(self.counts[len]);
      if code - count < first {
        return Ok(self.symbols[(index + (code - first)) as usize]);
      }
      index += count;
      first += count;
      first <<= 1;
      code <<= 1;
    }
    Err(Error::Invalid("a Huffman code that is not in the table"))
  }
}

fn reverse(
  code: u32,
  len: u32,
) -> u32 {
  code.reverse_bits() >> (32 - len)
}

const LENGTH_BASE: [u16; 29] = [
  3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67,
  83, 99, 115, 131, 163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
  0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5,
  5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
  1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513,
  769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
  0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11,
  11, 12, 12, 13, 13,
];

/// Inflates raw deflate data. `limit` is the most bytes it may make, so a
/// small broken file cannot ask for gigabytes.
#[cfg(test)]
pub fn inflate(
  data: &[u8],
  limit: usize,
) -> Result<Vec<u8>, Error> {
  let mut bits = Bits::new(data);
  let mut out = Vec::new();
  inflate_into(&mut bits, &mut out, limit)?;
  Ok(out)
}

fn inflate_into(
  bits: &mut Bits<'_>,
  out: &mut Vec<u8>,
  limit: usize,
) -> Result<(), Error> {
  loop {
    let last = bits.take(1)? == 1;
    match bits.take(2)? {
      0 => {
        bits.align();
        let len = bits.take(16)? as u16;
        let nlen = bits.take(16)? as u16;
        if len != !nlen {
          return Err(Error::Invalid("a stored block with a bad length"));
        }
        if out.len() + usize::from(len) > limit {
          return Err(Error::TooLarge);
        }
        bits.bytes(usize::from(len), out)?;
      },
      1 => {
        let mut lengths = [0u8; 288 + 32];
        lengths[..144].fill(8);
        lengths[144..256].fill(9);
        lengths[256..280].fill(7);
        lengths[280..288].fill(8);
        lengths[288..].fill(5);
        let lit = Huffman::new(&lengths[..288])?;
        let dist = Huffman::new(&lengths[288..])?;
        codes(bits, out, &lit, &dist, limit)?;
      },
      2 => {
        let (lit, dist) = dynamic(bits)?;
        codes(bits, out, &lit, &dist, limit)?;
      },
      _ => return Err(Error::Invalid("a deflate block of an unknown kind")),
    }
    if last {
      return Ok(());
    }
  }
}

fn dynamic(bits: &mut Bits<'_>) -> Result<(Huffman, Huffman), Error> {
  const ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
  ];
  let nlen = bits.take(5)? as usize + 257;
  let ndist = bits.take(5)? as usize + 1;
  let ncode = bits.take(4)? as usize + 4;
  if nlen > 286 || ndist > 30 {
    return Err(Error::Invalid("a deflate block with too many codes"));
  }
  let mut code_lengths = [0u8; 19];
  for &i in &ORDER[..ncode] {
    code_lengths[i] = bits.take(3)? as u8;
  }
  let code = Huffman::new(&code_lengths)?;
  let mut lengths = vec![0u8; nlen + ndist];
  let mut i = 0;
  while i < lengths.len() {
    let symbol = code.decode(bits)?;
    let (value, repeat) = match symbol {
      0..=15 => (symbol as u8, 1),
      16 => {
        let previous = *lengths
          .get(i.wrapping_sub(1))
          .ok_or(Error::Invalid("a repeat with nothing before it"))?;
        (previous, 3 + bits.take(2)? as usize)
      },
      17 => (0, 3 + bits.take(3)? as usize),
      18 => (0, 11 + bits.take(7)? as usize),
      _ => return Err(Error::Invalid("a bad code length code")),
    };
    if i + repeat > lengths.len() {
      return Err(Error::Invalid("code lengths that run past the end"));
    }
    lengths[i..i + repeat].fill(value);
    i += repeat;
  }
  if lengths[256] == 0 {
    return Err(Error::Invalid("a deflate block with no end code"));
  }
  Ok((
    Huffman::new(&lengths[..nlen])?,
    Huffman::new(&lengths[nlen..])?,
  ))
}

fn codes(
  bits: &mut Bits<'_>,
  out: &mut Vec<u8>,
  lit: &Huffman,
  dist: &Huffman,
  limit: usize,
) -> Result<(), Error> {
  loop {
    let symbol = lit.decode(bits)?;
    match symbol {
      0..=255 => {
        if out.len() >= limit {
          return Err(Error::TooLarge);
        }
        out.push(symbol as u8);
      },
      256 => return Ok(()),
      257..=285 => {
        let s = usize::from(symbol - 257);
        let len = usize::from(LENGTH_BASE[s])
          + bits.take(u32::from(LENGTH_EXTRA[s]))? as usize;
        let d = usize::from(dist.decode(bits)?);
        if d >= 30 {
          return Err(Error::Invalid("a bad distance code"));
        }
        let distance = usize::from(DIST_BASE[d])
          + bits.take(u32::from(DIST_EXTRA[d]))? as usize;
        if distance > out.len() {
          return Err(Error::Invalid("a distance before the start"));
        }
        if out.len() + len > limit {
          return Err(Error::TooLarge);
        }
        let from = out.len() - distance;
        if distance >= len {
          out.extend_from_within(from..from + len);
        } else {
          for k in 0..len {
            let b = out[from + k];
            out.push(b);
          }
        }
      },
      _ => return Err(Error::Invalid("a bad length code")),
    }
  }
}

/// Inflates zlib data, making at most `limit` bytes: a two-byte header, deflate data and an Adler-32
/// check of what it makes.
pub fn zlib(
  data: &[u8],
  limit: usize,
) -> Result<Vec<u8>, Error> {
  if data.len() < 2 {
    return Err(Error::Truncated);
  }
  let (cmf, flg) = (data[0], data[1]);
  if cmf & 15 != 8 || (u16::from(cmf) << 8 | u16::from(flg)) % 31 != 0 {
    return Err(Error::Invalid("a bad zlib header"));
  }
  if flg & 0x20 != 0 {
    return Err(Error::Invalid("a zlib stream with a preset dictionary"));
  }
  let mut bits = Bits::new(&data[2..]);
  let mut out = Vec::new();
  inflate_into(&mut bits, &mut out, limit)?;
  // The check is the four bytes after the deflate data.
  bits.align();
  let mut check = Vec::with_capacity(4);
  if bits.bytes(4, &mut check).is_ok() {
    let want = u32::from_be_bytes([check[0], check[1], check[2], check[3]]);
    if adler32(&out) != want {
      return Err(Error::Invalid("a zlib check that does not match"));
    }
  }
  Ok(out)
}

/// The Adler-32 check of `data`.
pub fn adler32(data: &[u8]) -> u32 {
  let (mut a, mut b) = (1u32, 0u32);
  for chunk in data.chunks(5552) {
    for &byte in chunk {
      a += u32::from(byte);
      b += a;
    }
    a %= 65521;
    b %= 65521;
  }
  b << 16 | a
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn stored_blocks_are_copied() {
    // One last stored block of "hello".
    let data = [
      1, 5, 0, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o',
    ];
    assert_eq!(inflate(&data, 100).unwrap(), b"hello");
  }

  #[test]
  fn fixed_codes_with_a_repeat() {
    // "abcabcabcabc" made by zlib with fixed codes.
    let data = [
      120, 218, 75, 76, 74, 78, 132, 33, 0, 29, 224, 4, 153,
    ];
    assert_eq!(zlib(&data, 100).unwrap(), b"abcabcabcabc");
  }

  #[test]
  fn limits_and_bad_data_are_errors() {
    let data = [
      1, 5, 0, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o',
    ];
    assert!(matches!(inflate(&data, 3), Err(Error::TooLarge)));
    assert!(inflate(&data[..7], 100).is_err());
    assert!(inflate(&[0xff, 0xff], 100).is_err());
  }

  #[test]
  fn adler32_matches_the_rfc_example() {
    assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
  }
}
