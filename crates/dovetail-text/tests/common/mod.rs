//! A tiny TrueType font built in code, so every value in it is known.
//! Shared by the font tests.

#![allow(dead_code)]

pub fn be16(v: u16) -> [u8; 2] {
  v.to_be_bytes()
}

/// A simple glyph: one square contour from (x0, y0) to (x1, y1).
pub fn square_glyph(
  x0: i16,
  y0: i16,
  x1: i16,
  y1: i16,
) -> Vec<u8> {
  let mut g = Vec::new();
  g.extend(be16(1)); // one contour
  for v in [x0, y0, x1, y1] {
    g.extend(be16(v as u16));
  }
  g.extend(be16(3)); // the contour ends at point 3
  g.extend(be16(0)); // no instructions
  g.extend([0x01; 4]); // four on-curve points, two-byte coordinates
  for (dx, dy) in [
    (x0, y0),
    (x1 - x0, 0),
    (0, y1 - y0),
    (x0 - x1, 0),
  ] {
    let _ = dy;
    g.extend(be16(dx as u16));
  }
  for dy in [y0, 0, y1 - y0, 0] {
    g.extend(be16(dy as u16));
  }
  g
}

/// A composite glyph: glyph 1 moved by (dx, dy).
pub fn moved_glyph(
  dx: i16,
  dy: i16,
) -> Vec<u8> {
  let mut g = Vec::new();
  g.extend(be16(0xffff)); // -1: composite
  g.extend([0u8; 8]); // bounds (unused by the reader for composites)
  g.extend(be16(0x0001 | 0x0002)); // word arguments, x and y values
  g.extend(be16(1)); // glyph 1
  g.extend(be16(dx as u16));
  g.extend(be16(dy as u16));
  g
}

/// The tables of a font with three glyphs: 0 (missing, empty), 1 (a square
/// for 'A') and 2 (the square moved, for 'B').
pub fn tables() -> Vec<([u8; 4], Vec<u8>)> {
  let glyphs = [
    Vec::new(),
    square_glyph(100, 0, 500, 700),
    moved_glyph(1000, 100),
  ];
  let mut glyf = Vec::new();
  let mut loca = Vec::new();
  for g in &glyphs {
    loca.extend(be16((glyf.len() / 2) as u16));
    glyf.extend(g);
    if glyf.len() % 2 == 1 {
      glyf.push(0);
    }
  }
  loca.extend(be16((glyf.len() / 2) as u16));

  let mut head = vec![0u8; 54];
  head[18..20].copy_from_slice(&be16(1000)); // units per em
  head[50..52].copy_from_slice(&be16(0)); // short loca
  let mut hhea = vec![0u8; 36];
  hhea[4..6].copy_from_slice(&be16(800)); // ascender
  hhea[6..8].copy_from_slice(&be16((-200i16) as u16)); // descender
  hhea[8..10].copy_from_slice(&be16(100)); // line gap
  hhea[34..36].copy_from_slice(&be16(3)); // advances for every glyph
  let mut maxp = vec![0u8; 6];
  maxp[4..6].copy_from_slice(&be16(3));
  let mut hmtx = Vec::new();
  for advance in [500u16, 600, 1600] {
    hmtx.extend(be16(advance));
    hmtx.extend(be16(0));
  }

  // cmap, format 4: 'A' (65) to 'B' (66) map to glyphs 1 and 2, then the
  // required final segment.
  let mut sub = Vec::new();
  let segs = 2u16;
  sub.extend(be16(4));
  sub.extend(be16(16 + 8 * segs)); // length
  sub.extend(be16(0));
  sub.extend(be16(segs * 2));
  sub.extend([0u8; 6]); // search fields (unused)
  for end in [66u16, 0xffff] {
    sub.extend(be16(end));
  }
  sub.extend(be16(0));
  for start in [65u16, 0xffff] {
    sub.extend(be16(start));
  }
  for delta in [(1i16 - 65) as u16, 1] {
    sub.extend(be16(delta));
  }
  for _ in 0..segs {
    sub.extend(be16(0));
  }
  let mut cmap = Vec::new();
  cmap.extend(be16(0));
  cmap.extend(be16(1));
  cmap.extend(be16(3));
  cmap.extend(be16(1));
  cmap.extend(12u32.to_be_bytes());
  cmap.extend(sub);

  // name: family "Test Sans" for Windows, English (US).
  let family: Vec<u8> = "Test Sans"
    .encode_utf16()
    .flat_map(u16::to_be_bytes)
    .collect();
  let mut name = Vec::new();
  name.extend(be16(0));
  name.extend(be16(1));
  name.extend(be16(6 + 12));
  for v in [
    3u16,
    1,
    0x0409,
    1,
    family.len() as u16,
    0,
  ] {
    name.extend(be16(v));
  }
  name.extend(family);

  vec![
    (*b"cmap", cmap),
    (*b"glyf", glyf),
    (*b"head", head),
    (*b"hhea", hhea),
    (*b"hmtx", hmtx),
    (*b"loca", loca),
    (*b"maxp", maxp),
    (*b"name", name),
  ]
}

/// A font file whose tables start `base` bytes into the data.
pub fn sfnt(base: usize) -> Vec<u8> {
  sfnt_with(base, Vec::new())
}

/// The test font with `extra` tables added.
pub fn sfnt_with(
  base: usize,
  extra: Vec<([u8; 4], Vec<u8>)>,
) -> Vec<u8> {
  let mut tables = tables();
  tables.extend(extra);
  sfnt_from(0x0001_0000, base, &tables)
}

/// A font file with this version tag (`0x00010000` for TrueType, `OTTO`
/// for CFF) and these tables, starting `base` bytes into the data.
pub fn sfnt_from(
  version: u32,
  base: usize,
  tables: &[([u8; 4], Vec<u8>)],
) -> Vec<u8> {
  let mut out = Vec::new();
  out.extend(version.to_be_bytes());
  out.extend(be16(tables.len() as u16));
  out.extend([0u8; 6]);
  let mut offset = base + 12 + 16 * tables.len();
  let mut body = Vec::new();
  for (tag, data) in tables {
    out.extend(tag);
    out.extend([0u8; 4]); // checksum (unused)
    out.extend((offset as u32).to_be_bytes());
    out.extend((data.len() as u32).to_be_bytes());
    body.extend(data);
    while body.len() % 4 != 0 {
      body.push(0);
    }
    offset = base + 12 + 16 * tables.len() + body.len();
  }
  out.extend(body);
  out
}
