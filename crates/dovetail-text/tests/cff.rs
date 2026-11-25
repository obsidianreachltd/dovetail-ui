//! CFF outlines from tables built in code, so every value is known (M06
//! step 3): an ordinary font, and a CID-keyed one whose glyphs are split
//! into groups with their own subroutines.

mod common;

use common::{sfnt, sfnt_from, tables};
use dovetail_text::{Font, Point, Segment, rasterize};

// Charstring operators.
const HSTEM: u8 = 1;
const RLINETO: u8 = 5;
const HLINETO: u8 = 6;
const VLINETO: u8 = 7;
const RRCURVETO: u8 = 8;
const CALLSUBR: u8 = 10;
const RETURN: u8 = 11;
const ENDCHAR: u8 = 14;
const HINTMASK: u8 = 19;
const RMOVETO: u8 = 21;
const CALLGSUBR: u8 = 29;

/// A charstring number.
fn n(v: i32) -> Vec<u8> {
  match v {
    -107..=107 => vec![(v + 139) as u8],
    108..=1131 => {
      let w = v - 108;
      vec![(w / 256 + 247) as u8, (w % 256) as u8]
    },
    -1131..=-108 => {
      let w = -v - 108;
      vec![(w / 256 + 251) as u8, (w % 256) as u8]
    },
    _ => {
      let mut b = vec![28];
      b.extend((v as i16).to_be_bytes());
      b
    },
  }
}

/// Numbers then an operator.
fn op(
  numbers: &[i32],
  operator: u8,
) -> Vec<u8> {
  let mut b: Vec<u8> = numbers.iter().flat_map(|&v| n(v)).collect();
  b.push(operator);
  b
}

/// A DICT integer in its five-byte form, so sizes are fixed.
fn int(v: usize) -> Vec<u8> {
  let mut b = vec![29];
  b.extend((v as i32).to_be_bytes());
  b
}

/// An INDEX of byte strings, with two-byte offsets.
fn index(items: &[Vec<u8>]) -> Vec<u8> {
  let mut b = Vec::new();
  b.extend((items.len() as u16).to_be_bytes());
  if items.is_empty() {
    return b;
  }
  b.push(2);
  let mut offset = 1u16;
  b.extend(offset.to_be_bytes());
  for item in items {
    offset += item.len() as u16;
    b.extend(offset.to_be_bytes());
  }
  for item in items {
    b.extend(item);
  }
  b
}

/// Glyph 0: nothing. Glyph 1: the square of the TrueType test font, with a
/// width first. Glyph 2: hints, then a line from a local subroutine and a
/// curve from a global one.
fn charstrings() -> Vec<Vec<u8>> {
  let square = [
    op(&[600, 100, 0], RMOVETO),
    op(&[400], HLINETO),
    op(&[700], VLINETO),
    op(&[-400], HLINETO),
    op(&[], ENDCHAR),
  ]
  .concat();
  let mut hinted = [
    op(&[10, 20], HSTEM),
    op(&[30, 40], HINTMASK),
  ]
  .concat();
  hinted.push(0xff); // the mask: one byte for two stems
  hinted.extend(
    [
      op(&[0, 0], RMOVETO),
      op(&[-107], CALLSUBR),
      op(&[-107], CALLGSUBR),
      op(&[], ENDCHAR),
    ]
    .concat(),
  );
  vec![op(&[], ENDCHAR), square, hinted]
}

fn local_subrs() -> Vec<u8> {
  index(&[[op(&[100, 0], RLINETO), vec![RETURN]].concat()])
}

fn global_subrs() -> Vec<u8> {
  index(&[[
    op(&[0, 100, 100, 100, 100, 0], RRCURVETO),
    vec![RETURN],
  ]
  .concat()])
}

/// A CFF table. With `cid`, glyphs 0 and 1 are in group 0, which has no
/// subroutines, and glyph 2 is in group 1, which has them.
fn cff(cid: bool) -> Vec<u8> {
  let header = vec![1u8, 0, 4, 2];
  let names = index(&[b"Test".to_vec()]);
  let strings = index(&[]);
  let globals = global_subrs();
  let glyphs = index(&charstrings());
  // A Private DICT whose subroutines follow it straight away.
  let private_with_subrs = [int(6), vec![19]].concat();
  let top_len = if cid { 20 } else { 17 };
  let top_index_len = 2 + 1 + 4 + top_len;
  let charstrings_at =
    header.len() + names.len() + top_index_len + strings.len() + globals.len();
  let after = charstrings_at + glyphs.len();

  let (top, rest) = if !cid {
    let top = [
      int(charstrings_at),
      vec![17],
      int(private_with_subrs.len()),
      int(after),
      vec![18],
    ]
    .concat();
    (top, [private_with_subrs, local_subrs()].concat())
  } else {
    // Group 0 has an empty Private DICT; group 1 has subroutines.
    let private1_at = after;
    let fd_array_at =
      private1_at + private_with_subrs.len() + local_subrs().len();
    let fd0 = [int(0), int(0), vec![18]].concat();
    let fd1 = [
      int(private_with_subrs.len()),
      int(private1_at),
      vec![18],
    ]
    .concat();
    let fd_array = index(&[fd0, fd1]);
    let fd_select_at = fd_array_at + fd_array.len();
    let mut fd_select = vec![3u8];
    fd_select.extend(2u16.to_be_bytes());
    fd_select.extend(0u16.to_be_bytes());
    fd_select.push(0);
    fd_select.extend(2u16.to_be_bytes());
    fd_select.push(1);
    fd_select.extend(3u16.to_be_bytes()); // the end: the glyph count
    let top = [
      int(charstrings_at),
      vec![17],
      int(fd_array_at),
      vec![12, 36],
      int(fd_select_at),
      vec![12, 37],
    ]
    .concat();
    let rest = [
      private_with_subrs,
      local_subrs(),
      fd_array,
      fd_select,
    ]
    .concat();
    (top, rest)
  };
  assert_eq!(top.len(), top_len);
  let mut table = header;
  table.extend(names);
  table.extend(index(&[top]));
  table.extend(strings);
  table.extend(globals);
  assert_eq!(table.len(), charstrings_at);
  table.extend(glyphs);
  table.extend(rest);
  table
}

/// A font file with CFF outlines in place of `glyf` and `loca`.
fn otto(table: Vec<u8>) -> Vec<u8> {
  let mut t: Vec<([u8; 4], Vec<u8>)> = tables()
    .into_iter()
    .filter(|(tag, _)| tag != b"glyf" && tag != b"loca")
    .collect();
  t.push((*b"CFF ", table));
  sfnt_from(u32::from_be_bytes(*b"OTTO"), 0, &t)
}

fn cff_font(cid: bool) -> Font {
  Font::from_data(otto(cff(cid)), 0).expect("the CFF test font")
}

fn p(
  x: f32,
  y: f32,
) -> Point {
  Point::new(x, y)
}

#[test]
fn draws_lines_and_skips_the_width() {
  let font = cff_font(false);
  let square = font.outline(1);
  assert_eq!(
    square.segments,
    [
      Segment::Line(p(100.0, 0.0), p(500.0, 0.0)),
      Segment::Line(p(500.0, 0.0), p(500.0, 700.0)),
      Segment::Line(p(500.0, 700.0), p(100.0, 700.0)),
      Segment::Line(p(100.0, 700.0), p(100.0, 0.0)),
    ]
  );
  assert_eq!(square.bounds, (p(100.0, 0.0), p(500.0, 700.0)));
  assert!(font.outline(0).is_empty());
}

#[test]
fn draws_the_same_pixels_as_the_truetype_square() {
  let truetype = Font::from_data(sfnt(0), 0).unwrap();
  let cff = cff_font(false);
  for size in [10.0, 17.0, 40.0] {
    let scale = size / 1000.0;
    assert_eq!(
      rasterize(&cff.outline(1), scale, 0.3),
      rasterize(&truetype.outline(1), scale, 0.3),
      "at {size} pixels"
    );
  }
}

#[test]
fn skips_hints_and_runs_subroutines() {
  let expected = [
    Segment::Line(p(0.0, 0.0), p(100.0, 0.0)),
    Segment::Cubic(
      p(100.0, 0.0),
      p(100.0, 100.0),
      p(200.0, 200.0),
      p(300.0, 200.0),
    ),
    Segment::Line(p(300.0, 200.0), p(0.0, 0.0)),
  ];
  assert_eq!(cff_font(false).outline(2).segments, expected);
  // In the CID-keyed font, glyph 2 uses its own group's subroutines.
  let cid = cff_font(true);
  assert_eq!(cid.outline(2).segments, expected);
  assert_eq!(cid.outline(1).segments.len(), 4);
}

#[test]
fn damaged_cff_data_never_panics() {
  for cid in [false, true] {
    let good = cff(cid);
    for at in 0..good.len() {
      for change in [0x01, 0xa5, 0xff] {
        let mut table = good.clone();
        table[at] ^= change;
        if let Ok(font) = Font::from_data(otto(table), 0) {
          for g in 0..3 {
            let _ = font.outline(g);
          }
        }
      }
    }
  }
}

#[test]
fn cff2_variable_fonts_are_refused_clearly() {
  let mut t: Vec<([u8; 4], Vec<u8>)> = tables()
    .into_iter()
    .filter(|(tag, _)| tag != b"glyf" && tag != b"loca")
    .collect();
  t.push((*b"CFF2", vec![2, 0, 5, 0, 0]));
  let data = sfnt_from(u32::from_be_bytes(*b"OTTO"), 0, &t);
  assert_eq!(
    Font::from_data(data, 0).unwrap_err(),
    dovetail_text::FontError::UnsupportedOutlines
  );
}
