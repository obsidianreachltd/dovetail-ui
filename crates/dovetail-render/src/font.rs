//! The built-in M04 font (D0012): printable ASCII drawn as strokes.
//!
//! Each glyph is a set of polylines on a grid 4 units wide and 8 high. Row 0
//! is the top of capitals, row 2 the top of small letters, row 6 the
//! baseline, and row 8 the bottom of descenders. A glyph string lists
//! polylines separated by spaces; each polyline is a run of two-digit
//! points, `xy`. A one-point polyline is a dot.
//!
//! Every character is 0.6 em wide and a line is 1.25 em high, the same as
//! `dovetail_layout::FixedPitch`. M06 replaces this font with real ones.

/// Width of each character, as a share of the font size.
pub const ADVANCE: f32 = 0.6;
/// Height of a line, as a share of the font size.
pub const LINE: f32 = 1.25;

/// Glyphs for characters 0x20 (space) to 0x7e (`~`).
const GLYPHS: [&str; 95] = [
  "",                               // space
  "2024 26",                        // !
  "1012 3032",                      // "
  "1016 3036 0242 0444",            // #
  "40100102133344453606 2028",      // $
  "4006 0010110100 3545463635",     // %
  "4612020110211224 2406064534",    // &
  "2022",                           // '
  "30212536",                       // (
  "10212516",                       // )
  "1135 3115 0343",                 // *
  "2125 0343",                      // +
  "2617",                           // ,
  "0343",                           // -
  "26",                             // .
  "4006",                           // /
  "0040460600 4006",                // 0
  "112026 1636",                    // 1
  "004043030646",                   // 2
  "00404606 1343",                  // 3
  "000343 4046",                    // 4
  "400003434606",                   // 5
  "400006464303",                   // 6
  "004016",                         // 7
  "0040460600 0343",                // 8
  "430300404606",                   // 9
  "22 26",                          // :
  "22 2617",                        // ;
  "410345",                         // <
  "0242 0444",                      // =
  "014305",                         // >
  "0040422324 26",                  // ?
  "4222244440000646",               // @
  "062046 1434",                    // A
  "0006 003041423303 033344453606", // B
  "40000646",                       // C
  "00304145360600",                 // D
  "40000646 0333",                  // E
  "400006 0333",                    // F
  "400006464323",                   // G
  "0006 4046 0343",                 // H
  "0040 2026 0646",                 // I
  "40460605",                       // J
  "0006 400346",                    // K
  "000646",                         // L
  "0600234046",                     // M
  "06004640",                       // N
  "0040460600",                     // O
  "06003041423303",                 // P
  "0040460600 2446",                // Q
  "06003041423303 2346",            // R
  "40100102133344453606",           // S
  "0040 2026",                      // T
  "00064640",                       // U
  "002640",                         // V
  "0016233640",                     // W
  "0046 4006",                      // X
  "002340 2326",                    // Y
  "00400646",                       // Z
  "30101636",                       // [
  "0046",                           // backslash
  "10303616",                       // ]
  "032043",                         // ^
  "0747",                           // _
  "1021",                           // `
  "42020646 4246",                  // a
  "0006 02424606",                  // b
  "42020646",                       // c
  "4046 42020646",                  // d
  "044442020646",                   // e
  "40201116 0232",                  // f
  "460602424808",                   // g
  "0006 024246",                    // h
  "2226 20",                        // i
  "222808 20",                      // j
  "0006 420446",                    // k
  "10202636",                       // l
  "06024246 2226",                  // m
  "06024246",                       // n
  "0242460602",                     // o
  "0208 02424606",                  // p
  "4248 42020646",                  // q
  "0206 031242",                    // r
  "420204444606",                   // s
  "101636 0232",                    // t
  "02064642",                       // u
  "022642",                         // v
  "0216243642",                     // w
  "0246 4206",                      // x
  "0226 421808",                    // y
  "02420646",                       // z
  "30212213242536",                 // {
  "2028",                           // |
  "10212233242516",                 // }
  "03123443",                       // ~
];

/// Drawn instead of characters the font does not have.
const MISSING: &str = "0040460600";

/// The polylines of `c`, in grid units.
fn strokes(c: char) -> Vec<Vec<(f32, f32)>> {
  let spec = match c as u32 {
    0x20..=0x7e => GLYPHS[(c as u32 - 0x20) as usize],
    _ => MISSING,
  };
  spec
    .split_whitespace()
    .map(|line| {
      line
        .as_bytes()
        .chunks(2)
        .map(|p| (f32::from(p[0] - b'0'), f32::from(p[1] - b'0')))
        .collect()
    })
    .collect()
}

/// A glyph drawn at one pixel size: one coverage byte per pixel.
pub struct GlyphBitmap {
  /// Width in pixels.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
  /// Coverage from 0 (empty) to 255 (full), row by row.
  pub coverage: Vec<u8>,
}

/// Draws `c` for text of `pixel_size` pixels (font size × display scale).
/// The bitmap covers the whole character cell: 0.6 em wide, 1.25 em high.
pub fn rasterize(
  c: char,
  pixel_size: f32,
) -> GlyphBitmap {
  let width = (ADVANCE * pixel_size).ceil().max(1.0) as u32;
  let height = (LINE * pixel_size).ceil().max(1.0) as u32;
  let mut coverage = vec![0u8; (width * height) as usize];

  // One grid unit is 0.1 em. The grid starts 0.1 em from the left and
  // 0.2 em from the top, so the baseline is at 0.8 em.
  let unit = pixel_size * 0.1;
  let (left, top) = (pixel_size * 0.1, pixel_size * 0.2);
  let half_width = (pixel_size * 0.045).max(0.6);
  let segments: Vec<((f32, f32), (f32, f32))> = strokes(c)
    .into_iter()
    .flat_map(|line| {
      let points: Vec<(f32, f32)> = line
        .iter()
        .map(|&(x, y)| (left + x * unit, top + y * unit))
        .collect();
      if points.len() == 1 {
        vec![(points[0], points[0])]
      } else {
        points.windows(2).map(|w| (w[0], w[1])).collect()
      }
    })
    .collect();

  for y in 0..height {
    for x in 0..width {
      let p = (x as f32 + 0.5, y as f32 + 0.5);
      let distance = segments
        .iter()
        .map(|&(a, b)| distance_to_segment(p, a, b))
        .fold(f32::INFINITY, f32::min);
      let alpha = (half_width + 0.5 - distance).clamp(0.0, 1.0);
      coverage[(y * width + x) as usize] = (alpha * 255.0).round() as u8;
    }
  }
  GlyphBitmap { width, height, coverage }
}

fn distance_to_segment(
  p: (f32, f32),
  a: (f32, f32),
  b: (f32, f32),
) -> f32 {
  let (abx, aby) = (b.0 - a.0, b.1 - a.1);
  let length = abx * abx + aby * aby;
  let t = if length == 0.0 {
    0.0
  } else {
    (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / length).clamp(0.0, 1.0)
  };
  let (cx, cy) = (a.0 + t * abx - p.0, a.1 + t * aby - p.1);
  (cx * cx + cy * cy).sqrt()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_glyph_parses_and_stays_on_the_grid() {
    for code in 0x20u8..=0x7e {
      for line in strokes(code as char) {
        assert!(!line.is_empty(), "{:?}", code as char);
        for (x, y) in line {
          assert!(
            (0.0..=4.0).contains(&x) && (0.0..=8.0).contains(&y),
            "{:?} has ({x}, {y})",
            code as char
          );
        }
      }
    }
    for spec in GLYPHS {
      for line in spec.split_whitespace() {
        assert_eq!(line.len() % 2, 0, "odd point string {line:?}");
      }
    }
  }

  #[test]
  fn glyphs_fill_the_cell_size_used_for_measuring() {
    let g = rasterize('A', 20.0);
    assert_eq!((g.width, g.height), (12, 25));
    assert!(g.coverage.contains(&255), "strokes are drawn");
    assert!(
      rasterize(' ', 20.0)
        .coverage
        .iter()
        .all(|&c| c == 0),
      "space is empty"
    );
  }

  #[test]
  fn characters_outside_ascii_draw_a_box() {
    let missing = rasterize('é', 20.0);
    let box_ = rasterize('O', 20.0);
    assert_eq!(missing.coverage, box_.coverage);
  }
}
