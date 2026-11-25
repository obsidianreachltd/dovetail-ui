//! Pictures of what a [`TestUi`] paints, compared with saved ones (M20).
//!
//! The window is painted with the GPU, as a real window is, but with the
//! built-in font, so a picture is the same on every system. A test
//! compares an element's picture with a PNG saved beside its tests, in
//! `tests/pictures/`. When they differ, the picture painted now and a
//! picture of the difference are saved under `target/dovetail-pictures/`,
//! and the failure says where.
//!
//! To save new pictures, or replace old ones after a change on purpose,
//! run the tests with `DOVETAIL_UPDATE_PICTURES=1`, then look at every
//! picture that changed before committing it.
//!
//! This needs a GPU adapter. On a machine without one, install a software
//! one, such as Mesa's lavapipe on Linux.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use dovetail_image::Rgba;
use dovetail_layout::Rect;
use dovetail_render::{Gpu, PaintInput, Painter};

use crate::{Found, TestUi};

/// How far a channel may be from the saved picture's, out of 255, before
/// the pixel counts as different. GPUs round colours a little differently.
pub const TOLERANCE: u8 = 2;

/// The GPU and painter a [`TestUi`] paints pictures with, made when the
/// first picture is asked for.
pub(crate) struct Painting {
  gpu: Gpu,
  painter: Painter,
}

/// A picture: its size in physical pixels, and sRGB red, green, blue and
/// alpha bytes for each pixel, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
  /// Width in pixels.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
  /// Each pixel, row by row.
  pub pixels: Vec<[u8; 4]>,
}

impl Picture {
  /// The pixel at (`x`, `y`).
  pub fn at(
    &self,
    x: u32,
    y: u32,
  ) -> [u8; 4] {
    self.pixels[(y * self.width + x) as usize]
  }

  /// The picture as a PNG file.
  pub fn to_png(&self) -> Vec<u8> {
    dovetail_image::encode_png(&Rgba {
      width: self.width,
      height: self.height,
      pixels: self.pixels.iter().flatten().copied().collect(),
    })
  }

  /// Reads a PNG file.
  pub fn from_png(data: &[u8]) -> Result<Picture, String> {
    let rgba = dovetail_image::decode_rgba(data).map_err(|e| e.to_string())?;
    Ok(Picture {
      width: rgba.width,
      height: rgba.height,
      pixels: rgba.pixels.as_chunks::<4>().0.to_vec(),
    })
  }

  /// The part of the picture inside `rect`, in physical pixels: from the
  /// pixel that `rect` starts in to the one it ends in.
  fn crop(
    &self,
    rect: Rect,
  ) -> Picture {
    let x0 = (rect.x.floor().max(0.0) as u32).min(self.width);
    let y0 = (rect.y.floor().max(0.0) as u32).min(self.height);
    let x1 = (((rect.x + rect.width).ceil().max(0.0)) as u32).min(self.width);
    let y1 = (((rect.y + rect.height).ceil().max(0.0)) as u32).min(self.height);
    let (width, height) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in y0..y0 + height {
      for x in x0..x0 + width {
        pixels.push(self.at(x, y));
      }
    }
    Picture { width, height, pixels }
  }
}

/// Where a test package keeps its saved pictures: `tests/pictures/`.
fn saved_dir() -> PathBuf {
  let root = std::env::var_os("CARGO_MANIFEST_DIR")
    .map(PathBuf::from)
    .or_else(|| std::env::current_dir().ok())
    .unwrap_or_default();
  root.join("tests").join("pictures")
}

/// Where failed comparisons are saved: `target/dovetail-pictures/`, found
/// from where the test program is, or the temporary folder.
fn failed_dir() -> PathBuf {
  std::env::current_exe()
    .ok()
    .and_then(|exe| {
      exe
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "target"))
        .map(Path::to_path_buf)
    })
    .unwrap_or_else(std::env::temp_dir)
    .join("dovetail-pictures")
}

/// True when saved pictures are to be replaced, not compared.
fn updating() -> bool {
  std::env::var_os("DOVETAIL_UPDATE_PICTURES").is_some_and(|v| v != "0")
}

fn write(
  path: &Path,
  picture: &Picture,
) {
  if let Some(dir) = path.parent()
    && let Err(e) = std::fs::create_dir_all(dir)
  {
    panic!("could not make {}: {e}", dir.display());
  }
  if let Err(e) = std::fs::write(path, picture.to_png()) {
    panic!("could not save {}: {e}", path.display());
  }
}

/// A picture of where `now` differs from `saved`: different pixels in
/// red, the rest faded.
fn difference(
  now: &Picture,
  saved: &Picture,
  tolerance: u8,
) -> (Picture, usize) {
  let mut count = 0;
  let pixels = now
    .pixels
    .iter()
    .zip(&saved.pixels)
    .map(|(a, b)| {
      if differs(*a, *b, tolerance) {
        count += 1;
        [255, 0, 0, 255]
      } else {
        let grey = (u16::from(b[0]) + u16::from(b[1]) + u16::from(b[2])) / 3;
        let faded = 160 + (grey * 95 / 255) as u8;
        [faded, faded, faded, 255]
      }
    })
    .collect();
  (
    Picture {
      width: now.width,
      height: now.height,
      pixels,
    },
    count,
  )
}

fn differs(
  a: [u8; 4],
  b: [u8; 4],
  tolerance: u8,
) -> bool {
  a.iter()
    .zip(&b)
    .any(|(x, y)| x.abs_diff(*y) > tolerance)
}

impl TestUi {
  /// Paints the whole window as it is now, at the window's scale.
  ///
  /// # Panics
  ///
  /// When there is no GPU adapter.
  pub fn window_picture(&self) -> Picture {
    let mut painting = self.painting.borrow_mut();
    let Painting { gpu, painter } = painting.get_or_insert_with(|| {
      let gpu = Gpu::headless()
        .unwrap_or_else(|e| panic!("pictures need a GPU adapter: {e:?}"));
      let mut painter = Painter::new(&gpu);
      painter.use_built_in_font();
      Painting { gpu, painter }
    });
    let width = (self.size.0 * self.scale).ceil() as u32;
    let height = (self.size.1 * self.scale).ceil() as u32;
    let mut canvas = painter.canvas(gpu, width, height);
    let style = self.session.style();
    let bars = self.session.scrollbars();
    painter.paint(
      gpu,
      &mut canvas,
      &PaintInput {
        boxes: self.session.layout().boxes(),
        region: None,
        scale: self.scale,
        background: style.background,
        focus_ring: self.session.focus_ring(),
        caret_visible: self.session.caret_visible(),
        colors: style,
        scrollbars: &bars,
      },
    );
    Picture {
      width,
      height,
      pixels: painter.read_pixels(gpu, &canvas),
    }
  }

  /// Paints the window and keeps the part where `element` is.
  ///
  /// # Panics
  ///
  /// When there is no GPU adapter.
  pub fn picture(
    &self,
    element: &Found,
  ) -> Picture {
    let r = element.rect();
    let s = self.scale;
    self.window_picture().crop(Rect::new(
      r.x * s,
      r.y * s,
      r.width * s,
      r.height * s,
    ))
  }

  /// Compares `element`'s picture with the one saved as
  /// `tests/pictures/{name}.png`, within [`TOLERANCE`] for each channel.
  ///
  /// # Panics
  ///
  /// When they differ, or none is saved. The picture painted now, and a
  /// picture of the difference with what changed in red, are saved under
  /// `target/dovetail-pictures/`, and the message says where. With
  /// `DOVETAIL_UPDATE_PICTURES=1`, the picture is saved instead.
  #[track_caller]
  pub fn assert_picture(
    &self,
    element: &Found,
    name: &str,
  ) {
    self.assert_picture_within(element, name, TOLERANCE);
  }

  /// As [`TestUi::assert_picture`], with how far a channel may be from
  /// the saved one.
  ///
  /// # Panics
  ///
  /// As [`TestUi::assert_picture`].
  #[track_caller]
  pub fn assert_picture_within(
    &self,
    element: &Found,
    name: &str,
    tolerance: u8,
  ) {
    let picture = self.picture(element);
    compare(&picture, name, tolerance);
  }

  /// Compares the whole window's picture with the one saved as
  /// `tests/pictures/{name}.png`.
  ///
  /// # Panics
  ///
  /// As [`TestUi::assert_picture`].
  #[track_caller]
  pub fn assert_window_picture(
    &self,
    name: &str,
  ) {
    compare(&self.window_picture(), name, TOLERANCE);
  }
}

/// Compares `picture` with the saved one called `name`.
#[track_caller]
fn compare(
  picture: &Picture,
  name: &str,
  tolerance: u8,
) {
  assert!(
    !name.is_empty()
      && name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
    "a picture's name is letters, digits, - and _: {name:?}"
  );
  let saved_path = saved_dir().join(format!("{name}.png"));
  if updating() {
    write(&saved_path, picture);
    eprintln!("saved the picture {}", saved_path.display());
    return;
  }
  let failed = failed_dir();
  let now_path = failed.join(format!("{name}.png"));
  let diff_path = failed.join(format!("{name}.difference.png"));
  let saved = match std::fs::read(&saved_path) {
    Ok(data) => Picture::from_png(&data).unwrap_or_else(|e| {
      panic!(
        "could not read the saved picture {}: {e}",
        saved_path.display()
      )
    }),
    Err(_) => {
      write(&now_path, picture);
      panic!(
        "there is no saved picture {}. The picture painted now is {}. If it \
         is right, run the tests with DOVETAIL_UPDATE_PICTURES=1 to save it.",
        saved_path.display(),
        now_path.display()
      );
    },
  };
  if (saved.width, saved.height) != (picture.width, picture.height) {
    write(&now_path, picture);
    panic!(
      "the picture {name:?} is {} by {} pixels, but the saved one is {} by \
       {}. The picture painted now is {}.",
      picture.width,
      picture.height,
      saved.width,
      saved.height,
      now_path.display()
    );
  }
  let (diff, count) = difference(picture, &saved, tolerance);
  if count > 0 {
    write(&now_path, picture);
    write(&diff_path, &diff);
    panic!(
      "{count} of {} pixels in the picture {name:?} differ from {} by more \
       than {tolerance}. The picture painted now is {}, and the difference, \
       in red, is {}.",
      picture.pixels.len(),
      saved_path.display(),
      now_path.display(),
      diff_path.display()
    );
  }
}

/// The painting state a [`TestUi`] keeps.
pub(crate) type Cache = RefCell<Option<Painting>>;
