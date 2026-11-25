//! The decoders against files they did not write (M17, D0023):
//!
//! - PngSuite's basic images, committed (their licence allows it);
//! - PNG and JPEG files written by Windows' own encoders, committed, made by
//!   `tests/fixtures/make.ps1`;
//! - PNG files written here, in every colour type, depth, filter and with
//!   interlacing, by a small encoder that only stores;
//! - when `tools/images/` has them, the Go source tree's PNG and JPEG test
//!   files and PngSuite's `.sng` descriptions (see the M17 plan).

use std::path::{Path, PathBuf};

use dovetail_image::{Bitmap, Error, Rgba, decode_rgba};

fn here(file: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("tests")
    .join(file)
}

fn tools(file: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../tools/images")
    .join(file)
}

fn read(path: &Path) -> Option<Vec<u8>> {
  std::fs::read(path).ok()
}

#[test]
fn pngsuite_basic_images_decode_at_their_size() {
  let dir = here("pngsuite");
  let mut count = 0;
  for entry in std::fs::read_dir(&dir).unwrap() {
    let path = entry.unwrap().path();
    if path.extension().is_none_or(|e| e != "png") {
      continue;
    }
    let image = decode_rgba(&std::fs::read(&path).unwrap())
      .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!((image.width, image.height), (32, 32), "{}", path.display());
    count += 1;
  }
  assert_eq!(count, 15);
  // Spot checks, from PngSuite's own descriptions.
  let p = |name: &str, x: usize, y: usize| {
    let image =
      decode_rgba(&std::fs::read(here(&format!("pngsuite/{name}"))).unwrap())
        .unwrap();
    let i = (y * 32 + x) * 4;
    [
      image.pixels[i],
      image.pixels[i + 1],
      image.pixels[i + 2],
      image.pixels[i + 3],
    ]
  };
  // basn3p02: palette entry 3 is blue, and the first pixel uses it.
  assert_eq!(p("basn3p02.png", 0, 0), [0, 0, 255, 255]);
  // basn6a08 starts fully transparent at the top left.
  assert_eq!(p("basn6a08.png", 0, 0)[3], 0);
  // basn0g01 is black and white only.
  assert!((0..32).all(|x| matches!(p("basn0g01.png", x, 5)[0], 0 | 255)));
}

/// The source image make.ps1 draws.
fn formula(
  w: usize,
  h: usize,
) -> Vec<[u8; 3]> {
  (0..h)
    .flat_map(|y| {
      (0..w).map(move |x| {
        [
          (4 * x) as u8,
          (5 * y) as u8,
          (2 * (x + y)) as u8,
        ]
      })
    })
    .collect()
}

#[test]
fn a_png_from_windows_is_exact() {
  let image =
    decode_rgba(&std::fs::read(here("fixtures/gdi.png")).unwrap()).unwrap();
  assert_eq!((image.width, image.height), (64, 48));
  for (i, want) in formula(64, 48).iter().enumerate() {
    assert_eq!(&image.pixels[i * 4..i * 4 + 3], want, "pixel {i}");
    assert_eq!(image.pixels[i * 4 + 3], 255);
  }
}

/// The mean and largest difference between two sets of RGB bytes.
fn difference(
  a: &Rgba,
  b: &[[u8; 3]],
) -> (f64, u8) {
  let mut sum = 0u64;
  let mut most = 0u8;
  for (i, want) in b.iter().enumerate() {
    for (c, &w) in want.iter().enumerate() {
      let d = a.pixels[i * 4 + c].abs_diff(w);
      sum += u64::from(d);
      most = most.max(d);
    }
  }
  (sum as f64 / (b.len() * 3) as f64, most)
}

#[test]
fn a_jpeg_from_windows_decodes_as_windows_does() {
  let image =
    decode_rgba(&std::fs::read(here("fixtures/gdi.jpg")).unwrap()).unwrap();
  assert_eq!((image.width, image.height), (64, 48));
  let windows: Vec<[u8; 3]> = std::fs::read(here("fixtures/gdi-jpg.rgb"))
    .unwrap()
    .chunks(3)
    .map(|c| [c[0], c[1], c[2]])
    .collect();
  let (mean, most) = difference(&image, &windows);
  assert!(mean < 1.5 && most <= 12, "mean {mean}, largest {most}");
  // And it is near the image that was saved.
  let (mean, _) = difference(&image, &formula(64, 48));
  assert!(mean < 3.0, "mean {mean} from the source");
}

#[test]
fn exif_orientation_turns_a_jpeg_upright() {
  let file = std::fs::read(here("fixtures/gdi.jpg")).unwrap();
  let upright = decode_rgba(&file).unwrap();
  // An APP1 segment after SOI, saying "turn 90° clockwise to show" (6),
  // in big-endian TIFF.
  let mut tiff = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01".to_vec();
  tiff.extend([0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0]);
  tiff.extend([0, 0, 0, 0]);
  let mut turned_file = file[..2].to_vec();
  turned_file.extend([0xff, 0xe1]);
  turned_file.extend(((tiff.len() + 2) as u16).to_be_bytes());
  turned_file.extend(&tiff);
  turned_file.extend(&file[2..]);
  let turned = decode_rgba(&turned_file).unwrap();
  assert_eq!((turned.width, turned.height), (48, 64));
  // The stored top-left pixel is now at the top right.
  let (w, h) = (upright.width as usize, upright.height as usize);
  for y in 0..h {
    for x in 0..w {
      let from = (y * w + x) * 4;
      let (tx, ty) = (h - 1 - y, x);
      let to = (ty * h + tx) * 4;
      assert_eq!(upright.pixels[from..from + 4], turned.pixels[to..to + 4]);
    }
  }
}

/// A PNG written here: stored (not compressed) deflate, with the filter for
/// each row chosen in turn, so every filter is used.
mod encoder {
  pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
      c ^= u32::from(b);
      for _ in 0..8 {
        c = if c & 1 != 0 {
          0xedb8_8320 ^ (c >> 1)
        } else {
          c >> 1
        };
      }
    }
    !c
  }

  fn adler(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &d in data {
      a = (a + u32::from(d)) % 65521;
      b = (b + a) % 65521;
    }
    b << 16 | a
  }

  fn chunk(
    out: &mut Vec<u8>,
    kind: &[u8; 4],
    body: &[u8],
  ) {
    out.extend((body.len() as u32).to_be_bytes());
    let mut data = kind.to_vec();
    data.extend(body);
    out.extend(&data);
    out.extend(crc32(&data).to_be_bytes());
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

  /// Filters raw rows of `row` bytes with `bpp` bytes a pixel.
  fn filter(
    rows: &[Vec<u8>],
    bpp: usize,
    first_filter: usize,
  ) -> Vec<u8> {
    let mut out = Vec::new();
    let mut previous = vec![0u8; rows.first().map_or(0, Vec::len)];
    for (n, row) in rows.iter().enumerate() {
      let f = (first_filter + n) % 5;
      out.push(f as u8);
      for i in 0..row.len() {
        let a = if i >= bpp { row[i - bpp] } else { 0 };
        let b = previous[i];
        let c = if i >= bpp { previous[i - bpp] } else { 0 };
        let predicted = match f {
          0 => 0,
          1 => a,
          2 => b,
          3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
          _ => paeth(a, b, c),
        };
        out.push(row[i].wrapping_sub(predicted));
      }
      previous.clone_from(row);
    }
    out
  }

  /// Packs samples of `depth` bits into rows.
  fn pack(
    samples: &[u16],
    per_row: usize,
    depth: u8,
  ) -> Vec<Vec<u8>> {
    samples
      .chunks(per_row)
      .map(|row| {
        let mut bytes = Vec::new();
        match depth {
          16 => row
            .iter()
            .for_each(|s| bytes.extend(s.to_be_bytes())),
          8 => row.iter().for_each(|&s| bytes.push(s as u8)),
          d => {
            let mut acc = 0u16;
            let mut bits = 0;
            for &s in row {
              acc = acc << d | s;
              bits += d;
              if bits == 8 {
                bytes.push(acc as u8);
                acc = 0;
                bits = 0;
              }
            }
            if bits > 0 {
              bytes.push((acc << (8 - bits)) as u8);
            }
          },
        }
        bytes
      })
      .collect()
  }

  /// Writes a PNG of `samples` (`channels` for each pixel, row by row).
  #[allow(clippy::too_many_arguments)]
  pub fn png(
    w: usize,
    h: usize,
    color: u8,
    depth: u8,
    channels: usize,
    samples: &[u16],
    interlaced: bool,
    palette: &[[u8; 3]],
    trns: &[u8],
  ) -> Vec<u8> {
    let bpp = (channels * usize::from(depth)).div_ceil(8).max(1);
    let mut raw = Vec::new();
    let passes: Vec<(usize, usize, usize, usize)> = if interlaced {
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
    for (n, &(x0, y0, dx, dy)) in passes.iter().enumerate() {
      let mut pass = Vec::new();
      let mut pw = 0;
      for y in (y0..h).step_by(dy) {
        pw = 0;
        for x in (x0..w).step_by(dx) {
          pass.extend_from_slice(
            &samples[(y * w + x) * channels..(y * w + x + 1) * channels],
          );
          pw += 1;
        }
      }
      if pw == 0 || pass.is_empty() {
        continue;
      }
      raw.extend(filter(&pack(&pass, pw * channels, depth), bpp, n));
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, block) in blocks.iter().enumerate() {
      z.push(u8::from(i == blocks.len() - 1));
      let len = block.len() as u16;
      z.extend(len.to_le_bytes());
      z.extend((!len).to_le_bytes());
      z.extend(*block);
    }
    z.extend(adler(&raw).to_be_bytes());
    let mut out = vec![
      0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a,
    ];
    let mut ihdr = (w as u32).to_be_bytes().to_vec();
    ihdr.extend((h as u32).to_be_bytes());
    ihdr.extend([depth, color, 0, 0, u8::from(interlaced)]);
    chunk(&mut out, b"IHDR", &ihdr);
    if !palette.is_empty() {
      chunk(&mut out, b"PLTE", &palette.concat());
    }
    if !trns.is_empty() {
      chunk(&mut out, b"tRNS", trns);
    }
    // Split the data over two IDAT chunks, as encoders may.
    let half = z.len() / 2;
    chunk(&mut out, b"IDAT", &z[..half]);
    chunk(&mut out, b"IDAT", &z[half..]);
    chunk(&mut out, b"IEND", &[]);
    out
  }
}

/// A sample at `depth` bits as 8, as the decoder should scale it.
fn eight(
  v: u16,
  depth: u8,
) -> u8 {
  match depth {
    16 => ((u32::from(v) * 255 + 32767) / 65535) as u8,
    d => (u32::from(v) * 255 / ((1 << d) - 1)) as u8,
  }
}

#[test]
fn every_colour_type_depth_filter_and_interlacing() {
  // Odd sizes, so passes and rows end part-way through bytes.
  let (w, h) = (13usize, 11usize);
  let mut seed = 12345u32;
  let mut random = move || {
    seed = seed
      .wrapping_mul(1_103_515_245)
      .wrapping_add(12345);
    (seed >> 8) as u16
  };
  let kinds: [(u8, &[u8], usize); 5] = [
    (0, &[1, 2, 4, 8, 16], 1),
    (2, &[8, 16], 3),
    (3, &[1, 2, 4, 8], 1),
    (4, &[8, 16], 2),
    (6, &[8, 16], 4),
  ];
  for (color, depths, channels) in kinds {
    for &depth in depths {
      for interlaced in [false, true] {
        let max = if depth == 16 {
          u16::MAX
        } else {
          (1u16 << depth) - 1
        };
        let samples: Vec<u16> = (0..w * h * channels)
          .map(|_| (u32::from(random()) % (u32::from(max) + 1)) as u16)
          .collect();
        let palette: Vec<[u8; 3]> = (0..=max.min(255))
          .map(|i| {
            [
              i as u8,
              255 - i as u8,
              (i as u8).wrapping_mul(7),
            ]
          })
          .collect();
        let palette = if color == 3 { palette } else { Vec::new() };
        // Half the palette is partly transparent.
        let trns: Vec<u8> = if color == 3 {
          (0..palette.len() / 2)
            .map(|i| (i * 3) as u8)
            .collect()
        } else {
          Vec::new()
        };
        let file = encoder::png(
          w, h, color, depth, channels, &samples, interlaced, &palette, &trns,
        );
        let what =
          format!("colour {color}, depth {depth}, interlaced {interlaced}");
        let image =
          decode_rgba(&file).unwrap_or_else(|e| panic!("{what}: {e}"));
        for i in 0..w * h {
          let s = &samples[i * channels..(i + 1) * channels];
          let want = match color {
            0 => {
              let g = eight(s[0], depth);
              [g, g, g, 255]
            },
            2 => [
              eight(s[0], depth),
              eight(s[1], depth),
              eight(s[2], depth),
              255,
            ],
            3 => {
              let p = palette[usize::from(s[0])];
              let a = trns
                .get(usize::from(s[0]))
                .copied()
                .unwrap_or(255);
              [p[0], p[1], p[2], a]
            },
            4 => {
              let g = eight(s[0], depth);
              [g, g, g, eight(s[1], depth)]
            },
            _ => [
              eight(s[0], depth),
              eight(s[1], depth),
              eight(s[2], depth),
              eight(s[3], depth),
            ],
          };
          assert_eq!(image.pixels[i * 4..i * 4 + 4], want, "{what}, pixel {i}");
        }
      }
    }
  }
}

#[test]
fn broken_and_cut_short_pngs_are_errors() {
  let samples: Vec<u16> = (0..64 * 3).map(|i| i as u16 % 256).collect();
  let good = encoder::png(8, 8, 2, 8, 3, &samples, false, &[], &[]);
  assert!(decode_rgba(&good).is_ok());
  // Cut short at every length.
  for len in 8..good.len() - 12 {
    assert!(decode_rgba(&good[..len]).is_err(), "cut at {len}");
  }
  // A changed byte breaks a check.
  for at in [20, 40, good.len() - 20] {
    let mut bad = good.clone();
    bad[at] ^= 0x55;
    assert!(decode_rgba(&bad).is_err(), "changed at {at}");
  }
  assert_eq!(decode_rgba(b"BM\0\0\0\0"), Err(Error::NotAnImage));
  assert_eq!(decode_rgba(b"GIF89a"), Err(Error::Truncated));
  assert!(Bitmap::decode(&[]).is_err());
}

// --- With files from tools/images/ (see the M17 plan) ----------------------

/// Reads a PngSuite `.sng` description into straight RGBA pixels.
fn sng(text: &str) -> Rgba {
  let field = |name: &str| -> usize {
    let at = text.find(&format!("{name}:")).unwrap() + name.len() + 1;
    text[at..]
      .trim_start()
      .split(|c: char| !c.is_ascii_digit())
      .next()
      .unwrap()
      .parse()
      .unwrap()
  };
  let (w, h, depth) =
    (field("width"), field("height"), field("bitdepth") as u8);
  let header = &text[text.find("IHDR").unwrap()..text.find('}').unwrap()];
  let palette_type = header.contains("palette");
  let colour = header.contains("color");
  let alpha = header.contains("alpha");
  let channels = match (palette_type, colour, alpha) {
    (true, _, _) => 1,
    (_, true, true) => 4,
    (_, true, false) => 3,
    (_, false, true) => 2,
    _ => 1,
  };
  let block = |name: &str| -> Option<&str> {
    let start = text.find(&format!("\n{name} {{"))? + name.len() + 3;
    Some(&text[start..start + text[start..].find('}')?])
  };
  let triples = |s: &str| -> Vec<u16> {
    s.lines()
      .filter_map(|l| l.split('#').next())
      .flat_map(|l| {
        l.split(|c: char| !c.is_ascii_digit())
          .filter(|t| !t.is_empty())
          .map(|t| t.parse::<u16>().unwrap())
          .collect::<Vec<_>>()
      })
      .collect()
  };
  let palette: Vec<u16> = block("PLTE").map(triples).unwrap_or_default();
  let trns: Vec<u16> = block("tRNS").map(triples).unwrap_or_default();
  let image = &text[text.find("pixels hex").unwrap() + 10..];
  let hex: String = image[..image.find('}').unwrap()]
    .chars()
    .filter(char::is_ascii_hexdigit)
    .collect();
  let bytes: Vec<u8> = (0..hex.len() / 2)
    .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
    .collect();
  let row_bytes = (w * channels * usize::from(depth)).div_ceil(8);
  let mut pixels = Vec::new();
  for y in 0..h {
    let row = &bytes[y * row_bytes..(y + 1) * row_bytes];
    let sample = |n: usize| -> u16 {
      match depth {
        16 => u16::from_be_bytes([row[2 * n], row[2 * n + 1]]),
        8 => u16::from(row[n]),
        d => {
          let bit = n * usize::from(d);
          u16::from(
            (row[bit / 8] >> (8 - usize::from(d) - bit % 8)) & ((1 << d) - 1),
          )
        },
      }
    };
    for x in 0..w {
      let s = |i| sample(x * channels + i);
      let px: [u8; 4] = if palette_type {
        let i = usize::from(s(0));
        let a = trns.get(i).map_or(255, |&a| a as u8);
        [
          palette[i * 3] as u8,
          palette[i * 3 + 1] as u8,
          palette[i * 3 + 2] as u8,
          a,
        ]
      } else {
        let transparent = !trns.is_empty()
          && match channels {
            1 => trns[0] == s(0),
            3 => trns[..3] == [s(0), s(1), s(2)],
            _ => false,
          };
        let a = if transparent { 0 } else { 255 };
        match channels {
          1 => [eight(s(0), depth); 3]
            .into_iter()
            .chain([a])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
          2 => [
            eight(s(0), depth),
            eight(s(0), depth),
            eight(s(0), depth),
            eight(s(1), depth),
          ],
          3 => [
            eight(s(0), depth),
            eight(s(1), depth),
            eight(s(2), depth),
            a,
          ],
          _ => [
            eight(s(0), depth),
            eight(s(1), depth),
            eight(s(2), depth),
            eight(s(3), depth),
          ],
        }
      };
      pixels.extend(px);
    }
  }
  Rgba {
    width: w as u32,
    height: h as u32,
    pixels,
  }
}

#[test]
fn pngsuite_matches_its_descriptions() {
  let dir = tools("pngsuite");
  let Ok(entries) = std::fs::read_dir(&dir) else {
    eprintln!("skipped: no {}", dir.display());
    return;
  };
  let mut count = 0;
  for entry in entries {
    let path = entry.unwrap().path();
    if path.extension().is_none_or(|e| e != "sng") {
      continue;
    }
    let text = std::fs::read_to_string(&path).unwrap();
    // These were made from 8-bit copies, so they do not describe the file.
    let name = path
      .file_stem()
      .unwrap()
      .to_string_lossy()
      .to_string();
    if text.contains("bitdepth: 8")
      && (name.starts_with("basn0g0") && !name.contains("g08"))
    {
      continue;
    }
    if name == "basn4a08" {
      continue;
    }
    let want = sng(&text);
    let file = path.with_extension("png");
    let got = decode_rgba(&std::fs::read(&file).unwrap())
      .unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!((got.width, got.height), (want.width, want.height), "{name}");
    for i in 0..(got.width * got.height) as usize {
      let (g, w) = (
        &got.pixels[i * 4..i * 4 + 4],
        &want.pixels[i * 4..i * 4 + 4],
      );
      // A fully transparent pixel's colour does not matter.
      if w[3] == 0 && g[3] == 0 {
        continue;
      }
      assert_eq!(g, w, "{name}, pixel {i}");
    }
    count += 1;
  }
  assert!(count > 20, "{count} files checked");
}

#[test]
fn go_png_files_interlaced_and_broken() {
  let Some(plain) = read(&tools("go/gray-gradient.png")) else {
    eprintln!("skipped: no tools/images/go");
    return;
  };
  let interlaced = read(&tools("go/gray-gradient.interlaced.png")).unwrap();
  assert_eq!(
    decode_rgba(&plain).unwrap(),
    decode_rgba(&interlaced).unwrap()
  );
  let rgb = decode_rgba(&read(&tools("go/benchRGB.png")).unwrap()).unwrap();
  let rgb_i =
    decode_rgba(&read(&tools("go/benchRGB-interlace.png")).unwrap()).unwrap();
  assert_eq!(rgb, rgb_i);
  for bench in [
    "benchGray",
    "benchNRGBA-gradient",
    "benchNRGBA-opaque",
    "benchPaletted",
  ] {
    decode_rgba(&read(&tools(&format!("go/{bench}.png"))).unwrap())
      .unwrap_or_else(|e| panic!("{bench}: {e}"));
  }
  for broken in [
    "invalid-crc32",
    "invalid-trunc",
    "invalid-zlib",
  ] {
    let data = read(&tools(&format!("go/{broken}.png"))).unwrap();
    assert!(decode_rgba(&data).is_err(), "{broken}");
  }
}

#[test]
fn go_jpeg_files_match_their_png_decodings() {
  let Some(_) = read(&tools("go/video-001.png")) else {
    eprintln!("skipped: no tools/images/go");
    return;
  };
  let png = |name: &str| {
    decode_rgba(&read(&tools(&format!("go/{name}.png"))).unwrap()).unwrap()
  };
  let jpeg = |name: &str| {
    decode_rgba(&read(&tools(&format!("go/{name}.jpeg"))).unwrap())
      .unwrap_or_else(|e| panic!("{name}: {e}"))
  };
  // Go's decoder takes subsampled colour from the nearest sample.
  let nearest = |name: &str| {
    dovetail_image::decode_jpeg_nearest(
      &read(&tools(&format!("go/{name}.jpeg"))).unwrap(),
    )
    .unwrap()
  };
  let rgb = |image: &Rgba| -> Vec<[u8; 3]> {
    image
      .pixels
      .chunks(4)
      .map(|p| [p[0], p[1], p[2]])
      .collect()
  };
  for (name, reference) in [
    ("video-001", "video-001"),
    ("video-001.progressive", "video-001"),
    ("video-001.221212", "video-001.221212"),
    ("video-001.cmyk", "video-001.cmyk"),
    ("video-001.rgb", "video-001.rgb"),
    ("video-005.gray", "video-005.gray"),
  ] {
    let got = nearest(name);
    let want = png(reference);
    assert_eq!((got.width, got.height), (want.width, want.height), "{name}");
    let (mean, most) = difference(&got, &rgb(&want));
    eprintln!("{name}: mean {mean:.3}, largest {most}");
    assert!(
      mean < 1.0 && most <= 8,
      "{name}: mean {mean}, largest {most}"
    );
  }
  // Progressive and baseline files of the same quality hold the same
  // coefficients, so they decode the same.
  for kind in [
    "separate.dc.progression",
    "q50.410",
    "q50.411",
    "q50.420",
    "q50.422",
    "q50.440",
    "q50.444",
  ] {
    let base = jpeg(&format!("video-001.{kind}"));
    let progressive = jpeg(&format!("video-001.{kind}.progressive"));
    assert_eq!(base, progressive, "{kind}");
  }
  for kind in [
    "video-005.gray.q50",
    "video-005.gray.q50.2x2",
  ] {
    assert_eq!(jpeg(kind), jpeg(&format!("{kind}.progressive")), "{kind}");
  }
  // A file with restart markers, made at a lower quality than the others.
  // Bytes before a marker that are not data are skipped, as Go's decoder
  // does.
  let restart = read(&tools("go/video-001.restart2.jpeg")).unwrap();
  let plain = decode_rgba(&restart).unwrap();
  let (mean, _) = difference(&plain, &rgb(&png("video-001")));
  assert!(mean < 6.0, "restart2: mean {mean}");
  assert_eq!(&restart[2816..2818], &[0xff, 0xd1]);
  let junks: [&[u8]; 5] = [
    &[],
    &[0x00],
    &[0x61, 0x62, 0x63, 0xff, 0x00, 0x64],
    &[0xff],
    &[
      0xff, 0xff, 0xff, 0x00, 0xff, 0x00, 0x00, 0xff, 0xff, 0xff,
    ],
  ];
  for junk in junks {
    let mut data = restart[..2816].to_vec();
    data.extend(junk);
    data.extend(&restart[2816..]);
    let got = decode_rgba(&data).unwrap();
    let (mean, _) = difference(&got, &rgb(&plain));
    assert!(mean < 0.5, "junk {junk:?}: mean {mean}");
  }
  // A cut-short progressive file shows what it has.
  let cut = jpeg("video-001.progressive.truncated");
  let want = png("video-001.progressive.truncated");
  let (mean, _) = difference(&cut, &rgb(&want));
  assert!(mean < 3.0, "truncated: mean {mean}");
}

#[test]
fn damaged_files_never_panic() {
  let mut files = vec![
    std::fs::read(here("fixtures/gdi.jpg")).unwrap(),
    std::fs::read(here("fixtures/gdi.png")).unwrap(),
    std::fs::read(here("pngsuite/basn3p04.png")).unwrap(),
  ];
  if let Some(p) = read(&tools("go/video-001.progressive.jpeg")) {
    files.push(p);
  }
  let mut seed = 99u32;
  let mut random = move || {
    seed ^= seed << 13;
    seed ^= seed >> 17;
    seed ^= seed << 5;
    seed
  };
  for file in &files {
    for _ in 0..400 {
      let mut bad = file.clone();
      for _ in 0..1 + random() % 8 {
        let at = random() as usize % bad.len();
        bad[at] = random() as u8;
      }
      if random() % 4 == 0 {
        bad.truncate(random() as usize % bad.len());
      }
      // Ok or an error; never a panic.
      let _ = decode_rgba(&bad);
    }
  }
}

// --- Animation: GIF and APNG (D0024) ----------------------------------------

use std::time::Duration;

use dovetail_image::{Animation, ApngFrame, GifFrame, encode_apng, encode_gif};

/// A `w` × `h` image of `colour`, with the pixels where `hole` is true left
/// fully transparent.
fn solid(
  w: u32,
  h: u32,
  colour: [u8; 4],
  hole: impl Fn(u32, u32) -> bool,
) -> Rgba {
  let mut pixels = Vec::new();
  for y in 0..h {
    for x in 0..w {
      pixels.extend(if hole(x, y) { [0, 0, 0, 0] } else { colour });
    }
  }
  Rgba { width: w, height: h, pixels }
}

/// Draws `part` on `canvas` at (`x`, `y`): replacing, or over what is there.
fn put(
  canvas: &mut Rgba,
  part: &Rgba,
  x: u32,
  y: u32,
  over: bool,
) {
  for py in 0..part.height {
    for px in 0..part.width {
      let s = ((py * part.width + px) * 4) as usize;
      let d = (((y + py) * canvas.width + x + px) * 4) as usize;
      let src = &part.pixels[s..s + 4];
      if !over || src[3] == 255 {
        canvas.pixels[d..d + 4].copy_from_slice(src);
      } else if src[3] > 0 {
        let sa = f32::from(src[3]) / 255.0;
        let da = f32::from(canvas.pixels[d + 3]) / 255.0;
        let a = sa + da * (1.0 - sa);
        for (k, &sk) in src[..3].iter().enumerate() {
          let c = (f32::from(sk) * sa
            + f32::from(canvas.pixels[d + k]) * da * (1.0 - sa))
            / a;
          canvas.pixels[d + k] = (c + 0.5) as u8;
        }
        canvas.pixels[d + 3] = (a * 255.0 + 0.5) as u8;
      }
    }
  }
}

fn clear(
  canvas: &mut Rgba,
  x: u32,
  y: u32,
  w: u32,
  h: u32,
) {
  put(canvas, &solid(w, h, [0; 4], |_, _| true), x, y, false);
}

fn ms(n: u64) -> Duration {
  Duration::from_millis(n)
}

#[test]
fn gif_frames_are_drawn_over_each_other_and_disposed() {
  const RED: [u8; 4] = [255, 0, 0, 255];
  const BLUE: [u8; 4] = [0, 0, 255, 255];
  const GREEN: [u8; 4] = [0, 255, 0, 255];
  let frame = |image, left, top, delay, disposal, interlaced| GifFrame {
    image,
    left,
    top,
    delay: ms(delay),
    disposal,
    interlaced,
  };
  let frames = [
    // The whole canvas, red.
    frame(solid(20, 10, RED, |_, _| false), 0, 0, 50, 1, false),
    // A blue square with a transparent middle, cleared after.
    frame(
      solid(5, 5, BLUE, |x, y| x == 2 && y == 2),
      3,
      2,
      70,
      2,
      false,
    ),
    // A green bar, interlaced, put back as it was after.
    frame(solid(7, 9, GREEN, |_, y| y % 3 == 0), 10, 1, 0, 3, true),
    // A blue pixel over what is left.
    frame(solid(1, 1, BLUE, |_, _| false), 0, 0, 10, 1, false),
  ];
  let file = encode_gif(20, 10, &frames, Some(3)).expect("few colours");
  let animation = Animation::decode(&file).unwrap();
  assert_eq!(animation.plays(), Some(3));
  let got = animation.frames();
  assert_eq!(got.len(), 4);
  let delays: Vec<u128> = got.iter().map(|f| f.delay.as_millis()).collect();
  assert_eq!(delays, [50, 70, 100, 100], "too short to see is 100 ms");

  let mut canvas = solid(20, 10, [0; 4], |_, _| true);
  let mut want = Vec::new();
  put(&mut canvas, &frames[0].image, 0, 0, true);
  want.push(canvas.clone());
  put(&mut canvas, &frames[1].image, 3, 2, true);
  want.push(canvas.clone());
  clear(&mut canvas, 3, 2, 5, 5);
  let before = canvas.clone();
  put(&mut canvas, &frames[2].image, 10, 1, true);
  want.push(canvas.clone());
  canvas = before;
  put(&mut canvas, &frames[3].image, 0, 0, true);
  want.push(canvas);
  for (i, (g, w)) in got.iter().zip(&want).enumerate() {
    assert_eq!(&g.bitmap.to_rgba(), w, "frame {i}");
  }
  // A still decode shows the first frame.
  assert_eq!(decode_rgba(&file).unwrap(), want[0]);
  let forever = encode_gif(20, 10, &frames[..1], None).unwrap();
  assert_eq!(Animation::decode(&forever).unwrap().plays(), None);
  let once = encode_gif(20, 10, &frames[..1], Some(1)).unwrap();
  let once = Animation::decode(&once).unwrap();
  assert_eq!((once.plays(), once.is_animated()), (Some(1), false));
}

#[test]
fn apng_frames_blend_and_dispose() {
  let frame = |image, x, y, delay, dispose, blend| ApngFrame {
    image,
    x,
    y,
    delay: ms(delay),
    dispose,
    blend,
  };
  let frames = [
    frame(
      solid(16, 8, [10, 20, 30, 255], |_, _| false),
      0,
      0,
      40,
      0,
      0,
    ),
    // Half-transparent white drawn over, then cleared.
    frame(
      solid(4, 4, [255, 255, 255, 128], |_, _| false),
      2,
      2,
      60,
      1,
      1,
    ),
    // Replacing, holes and all, then put back as it was.
    frame(solid(5, 3, [200, 0, 0, 255], |x, _| x == 0), 9, 4, 5, 2, 0),
    frame(solid(2, 2, [0, 200, 0, 255], |_, _| false), 14, 6, 30, 0, 1),
  ];
  let file = encode_apng(16, 8, &frames, 0);
  let animation = Animation::decode(&file).unwrap();
  assert_eq!(animation.plays(), None, "0 is for ever");
  let got = animation.frames();
  assert_eq!(got.len(), 4);
  let delays: Vec<u128> = got.iter().map(|f| f.delay.as_millis()).collect();
  assert_eq!(delays, [40, 60, 100, 30]);
  let mut canvas = solid(16, 8, [0; 4], |_, _| true);
  let mut want = Vec::new();
  put(&mut canvas, &frames[0].image, 0, 0, false);
  want.push(canvas.clone());
  put(&mut canvas, &frames[1].image, 2, 2, true);
  want.push(canvas.clone());
  clear(&mut canvas, 2, 2, 4, 4);
  let before = canvas.clone();
  put(&mut canvas, &frames[2].image, 9, 4, false);
  want.push(canvas.clone());
  canvas = before;
  put(&mut canvas, &frames[3].image, 14, 6, true);
  want.push(canvas);
  for (i, (g, w)) in got.iter().zip(&want).enumerate() {
    let g = g.bitmap.to_rgba();
    for (k, (a, b)) in g.pixels.iter().zip(&w.pixels).enumerate() {
      // Half-transparent pixels go through premultiplying and back.
      assert!(a.abs_diff(*b) <= 2, "frame {i}, byte {k}: {a} not {b}");
    }
  }
  // Read as a plain PNG, it is its first frame.
  assert_eq!(decode_rgba(&file).unwrap(), want[0]);
}

#[test]
fn go_gif_files() {
  let Some(gray_gif) = read(&tools("go/video-005.gray.gif")) else {
    eprintln!("skipped: no tools/images/go");
    return;
  };
  // Greys in a palette: near the PNG.
  let gray_png =
    decode_rgba(&read(&tools("go/video-005.gray.png")).unwrap()).unwrap();
  let grey: Vec<[u8; 3]> = gray_png
    .pixels
    .chunks(4)
    .map(|p| [p[0], p[1], p[2]])
    .collect();
  let (mean, most) = difference(&decode_rgba(&gray_gif).unwrap(), &grey);
  eprintln!("video-005.gray.gif: mean {mean:.3}, largest {most}");
  assert!(mean < 3.0, "grey: mean {mean}");
  // Interlaced and not are the same picture.
  let plain = decode_rgba(&read(&tools("go/video-001.gif")).unwrap()).unwrap();
  let interlaced =
    decode_rgba(&read(&tools("go/video-001.interlaced.gif")).unwrap()).unwrap();
  assert_eq!(plain, interlaced);
  // Fewer colours than the PNG, but close to it.
  let png = decode_rgba(&read(&tools("go/video-001.png")).unwrap()).unwrap();
  let rgb: Vec<[u8; 3]> = png
    .pixels
    .chunks(4)
    .map(|p| [p[0], p[1], p[2]])
    .collect();
  let (mean, _) = difference(&plain, &rgb);
  eprintln!("video-001.gif: mean {mean:.3}");
  assert!(mean < 6.0, "mean {mean}");
  for other in ["video-001.5bpp", "triangle-001"] {
    decode_rgba(&read(&tools(&format!("go/{other}.gif"))).unwrap())
      .unwrap_or_else(|e| panic!("{other}: {e}"));
  }
}

#[test]
fn damaged_animations_never_panic() {
  let gif = encode_gif(
    9,
    7,
    &[
      GifFrame {
        image: solid(9, 7, [1, 2, 3, 255], |x, y| (x + y) % 4 == 0),
        left: 0,
        top: 0,
        delay: ms(20),
        disposal: 2,
        interlaced: true,
      },
      GifFrame {
        image: solid(3, 3, [9, 9, 9, 255], |_, _| false),
        left: 5,
        top: 3,
        delay: ms(20),
        disposal: 3,
        interlaced: false,
      },
    ],
    None,
  )
  .unwrap();
  let apng = encode_apng(
    9,
    7,
    &[
      ApngFrame {
        image: solid(9, 7, [1, 2, 3, 255], |_, _| false),
        x: 0,
        y: 0,
        delay: ms(20),
        dispose: 2,
        blend: 1,
      },
      ApngFrame {
        image: solid(3, 3, [9, 9, 9, 100], |_, _| false),
        x: 5,
        y: 3,
        delay: ms(20),
        dispose: 1,
        blend: 1,
      },
    ],
    2,
  );
  let mut seed = 7u32;
  let mut random = move || {
    seed ^= seed << 13;
    seed ^= seed >> 17;
    seed ^= seed << 5;
    seed
  };
  for file in [&gif, &apng] {
    for _ in 0..3000 {
      let mut bad = file.clone();
      for _ in 0..1 + random() % 6 {
        let at = random() as usize % bad.len();
        bad[at] = random() as u8;
      }
      if random() % 4 == 0 {
        bad.truncate(random() as usize % bad.len());
      }
      let _ = Animation::decode(&bad);
    }
  }
}
