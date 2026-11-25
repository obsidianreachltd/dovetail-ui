//! Draws every icon in Dovetail's set at several sizes into a PNG file, to
//! look at them: `cargo run -p dovetail-image --example icon_sheet --
//! icons.png`.

use dovetail_image::{Rgba, encode_png, icons};

fn main() {
  let path = std::env::args()
    .nth(1)
    .unwrap_or_else(|| "icons.png".to_owned());
  let sizes = [16u32, 20, 24, 32, 48];
  let cell = 56u32;
  let (w, h) = (cell * sizes.len() as u32, cell * icons::ALL.len() as u32);
  let mut pixels = vec![255u8; (w * h * 4) as usize];
  for (row, (_, icon)) in icons::ALL.iter().enumerate() {
    for (col, &size) in sizes.iter().enumerate() {
      let c = icon().render(size, size);
      let (x0, y0) = (
        col as u32 * cell + (cell - size) / 2,
        row as u32 * cell + (cell - size) / 2,
      );
      for y in 0..size {
        for x in 0..size {
          let a = f32::from(c.alpha[(y * size + x) as usize]) / 255.0;
          let at = (((y0 + y) * w + x0 + x) * 4) as usize;
          // Dark ink on white, blended in sRGB as text usually is.
          let v = (255.0 * (1.0 - a * 0.87)).round() as u8;
          pixels[at..at + 3].fill(v);
        }
      }
    }
  }
  std::fs::write(&path, encode_png(&Rgba { width: w, height: h, pixels }))
    .expect("write the sheet");
  println!(
    "wrote {path}: {} icons at {sizes:?} pixels",
    icons::ALL.len()
  );
}
