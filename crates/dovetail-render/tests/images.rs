//! M17: images and icons on the GPU.
//!
//! - An image drawn at its own pixel size is drawn pixel for pixel at 100%,
//!   150% and 200% display scale (criterion 1), and fits and fills a box.
//! - An icon is drawn at the display's size in the text's colour or its
//!   own, as sharp as text (criterion 2).
//! - A large image is uploaded over several frames, and textures go when
//!   their images do, or past the budget.
//!
//! These need a GPU adapter, so they do not run by default:
//! `cargo test -p dovetail-render --test images -- --ignored`.

use dovetail_core::{
  Align, BuildContext, Color, Component, Edges, Element, ElementKind, Fit,
  Size, Style, Tree, View,
};
use dovetail_image::{Bitmap, Rgba, icons};
use dovetail_layout::Layout;
use dovetail_render::{Gpu, PaintInput, PaintStats, Painter, font_measure};

struct Show;

#[derive(PartialEq)]
struct ShowProps {
  view: View,
}

impl Component for Show {
  type Props = ShowProps;

  fn build(
    props: &ShowProps,
    _: &mut BuildContext<'_>,
  ) -> View {
    props.view.clone()
  }
}

/// Lays out `view` in `logical` pixels and paints it at `scale` on white.
fn paint(
  gpu: &Gpu,
  painter: &mut Painter,
  view: View,
  logical: (f32, f32),
  scale: f32,
) -> (Vec<[u8; 4]>, u32, PaintStats) {
  let mut tree = Tree::new();
  tree.mount(View::component::<Show>(ShowProps { view }));
  let mut layout = Layout::new();
  layout.update(&tree, logical.0, logical.1, &font_measure());
  let (w, h) = (
    (logical.0 * scale).ceil() as u32,
    (logical.1 * scale).ceil() as u32,
  );
  let mut canvas = painter.canvas(gpu, w, h);
  let stats = painter.paint(
    gpu,
    &mut canvas,
    &PaintInput {
      background: Color::WHITE,
      ..PaintInput::new(layout.boxes(), scale)
    },
  );
  (painter.read_pixels(gpu, &canvas), w, stats)
}

/// A little image of colours that differ in every pixel, some partly
/// transparent.
fn pattern(
  w: u32,
  h: u32,
) -> Rgba {
  let mut pixels = Vec::new();
  for y in 0..h {
    for x in 0..w {
      pixels.extend([
        (x * 37 % 256) as u8,
        (y * 53 % 256) as u8,
        ((x * 11 + y * 29) % 256) as u8,
        255,
      ]);
    }
  }
  Rgba { width: w, height: h, pixels }
}

/// `view` with `padding` logical pixels before it, so it starts part way
/// through a pixel at 150%.
fn padded(
  view: View,
  padding: f32,
) -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges {
        left: padding,
        top: padding,
        right: 0.0,
        bottom: 0.0,
      },
      ..Style::default()
    })
    .children([view])
    .into()
}

#[test]
#[ignore = "needs a GPU adapter"]
fn images_drawn_at_their_size_are_pixel_for_pixel() {
  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  let source = pattern(30, 20);
  let bitmap = Bitmap::from_rgba(source.clone());
  for scale in [1.0f32, 1.5, 2.0] {
    // An image drawn for this scale: one image pixel to each screen pixel.
    let view = padded(bitmap.picture_at(scale).into(), 3.0);
    let (pixels, w, stats) =
      paint(&gpu, &mut painter, view, (60.0, 40.0), scale);
    assert_eq!(stats.images, 1);
    assert!(stats.waiting.is_none());
    let origin = (3.0 * scale).round() as u32;
    for y in 0..20 {
      for x in 0..30 {
        let got = pixels[((origin + y) * w + origin + x) as usize];
        let i = ((y * 30 + x) * 4) as usize;
        let want = &source.pixels[i..i + 3];
        for c in 0..3 {
          assert!(
            got[c].abs_diff(want[c]) <= 1,
            "scale {scale}, pixel ({x}, {y}): {got:?} not {want:?}"
          );
        }
      }
    }
    // And nothing outside it.
    assert_eq!(
      pixels[((origin + 20) * w + origin) as usize],
      [255, 255, 255, 255]
    );
    assert_eq!(
      pixels[(origin * w + origin + 30) as usize],
      [255, 255, 255, 255]
    );
  }
}

/// A 40 × 20 image: red on the left half, blue on the right.
fn halves() -> Bitmap {
  let mut pixels = Vec::new();
  for _ in 0..20 {
    for x in 0..40 {
      pixels.extend(if x < 20 {
        [255, 0, 0, 255]
      } else {
        [0, 0, 255, 255]
      });
    }
  }
  Bitmap::from_rgba(Rgba { width: 40, height: 20, pixels })
}

fn in_box(
  bitmap: &Bitmap,
  fit: Fit,
) -> View {
  Element::new(ElementKind::Stack)
    .style(Style {
      width: Size::Fixed(40.0),
      height: Size::Fixed(40.0),
      align: Align::Stretch,
      clip: true,
      ..Style::default()
    })
    .children([bitmap.picture().fit(fit).into()])
    .into()
}

#[test]
#[ignore = "needs a GPU adapter"]
fn images_fit_and_fill_a_box() {
  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  let bitmap = halves();
  let at =
    |pixels: &[[u8; 4]], w: u32, x: u32, y: u32| pixels[(y * w + x) as usize];
  const RED: [u8; 4] = [255, 0, 0, 255];
  const BLUE: [u8; 4] = [0, 0, 255, 255];
  const WHITE: [u8; 4] = [255, 255, 255, 255];

  // Contained: the whole image, as wide as the box, centred down it.
  let (p, w, _) = paint(
    &gpu,
    &mut painter,
    in_box(&bitmap, Fit::Contain),
    (40.0, 40.0),
    1.0,
  );
  assert_eq!(at(&p, w, 5, 5), WHITE);
  assert_eq!(at(&p, w, 5, 20), RED);
  assert_eq!(at(&p, w, 35, 20), BLUE);
  assert_eq!(at(&p, w, 35, 35), WHITE);

  // Covered: twice the size, so only its middle shows, half and half.
  let (p, w, _) = paint(
    &gpu,
    &mut painter,
    in_box(&bitmap, Fit::Cover),
    (40.0, 40.0),
    1.0,
  );
  assert_eq!(at(&p, w, 5, 2), RED);
  assert_eq!(at(&p, w, 35, 38), BLUE);

  // Stretched: the whole image, to the box's shape.
  let (p, w, _) = paint(
    &gpu,
    &mut painter,
    in_box(&bitmap, Fit::Stretch),
    (40.0, 40.0),
    1.0,
  );
  assert_eq!(at(&p, w, 5, 2), RED);
  assert_eq!(at(&p, w, 35, 38), BLUE);

  // Its own size: centred, as tall as it is.
  let (p, w, _) = paint(
    &gpu,
    &mut painter,
    in_box(&bitmap, Fit::None),
    (40.0, 40.0),
    1.0,
  );
  assert_eq!(at(&p, w, 5, 5), WHITE);
  assert_eq!(at(&p, w, 5, 15), RED);
  assert_eq!(at(&p, w, 35, 25), BLUE);
}

#[test]
#[ignore = "needs a GPU adapter"]
fn shrunk_images_are_smooth() {
  // Fine black and white stripes, shrunk to an eighth, average to the grey
  // of their light, not to black or white.
  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  let mut pixels = Vec::new();
  for _ in 0..64 {
    for x in 0..64 {
      let v = if x % 2 == 0 { 0 } else { 255 };
      pixels.extend([v, v, v, 255]);
    }
  }
  let bitmap = Bitmap::from_rgba(Rgba { width: 64, height: 64, pixels });
  let view: View = bitmap.picture().sized(8.0, 8.0).into();
  let (p, _, _) = paint(&gpu, &mut painter, view, (8.0, 8.0), 1.0);
  for px in &p {
    assert!((180..=196).contains(&px[0]), "{px:?}");
  }
}

/// The pixels an icon covers fully and not at all, at a size, as the
/// painter should place them.
fn check_icon(
  scale: f32,
  color: Option<Color>,
  ink: [u8; 3],
) {
  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  let icon = icons::save();
  let mut picture = icon.picture();
  if let Some(c) = color {
    picture = picture.color(c);
  }
  // In red text, 14 pixels high: the icon is 16 logical pixels.
  let view: View = Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges {
        left: 3.0,
        top: 5.0,
        right: 0.0,
        bottom: 0.0,
      },
      font_size: Some(14.0),
      text_color: Some(Color::rgb(1.0, 0.0, 0.0)),
      ..Style::default()
    })
    .children([picture.into()])
    .into();
  let (pixels, w, stats) = paint(&gpu, &mut painter, view, (30.0, 30.0), scale);
  assert_eq!(stats.glyphs, 1);
  let size = (16.0 * scale).round() as u32;
  let cover = icon.render(size, size);
  let (x0, y0) = ((3.0 * scale).round() as u32, (5.0 * scale).round() as u32);
  let (mut full, mut empty) = ((0, 0), (0, 0));
  for y in 0..size {
    for x in 0..size {
      let got = pixels[((y0 + y) * w + x0 + x) as usize];
      match cover.alpha[(y * size + x) as usize] {
        255 => {
          full.0 += 1;
          if (0..3).all(|c| got[c].abs_diff(ink[c]) <= 2) {
            full.1 += 1;
          }
        },
        0 => {
          empty.0 += 1;
          if got == [255, 255, 255, 255] {
            empty.1 += 1;
          }
        },
        _ => {},
      }
    }
  }
  assert!(full.0 > 20, "scale {scale}: {} solid pixels", full.0);
  assert_eq!(full.1, full.0, "scale {scale}: solid pixels in the ink");
  assert_eq!(empty.1, empty.0, "scale {scale}: empty pixels left alone");
}

#[test]
#[ignore = "needs a GPU adapter"]
fn icons_are_sharp_in_the_text_colour_at_every_scale() {
  for scale in [1.0, 1.5, 2.0] {
    check_icon(scale, None, [255, 0, 0]);
  }
}

#[test]
#[ignore = "needs a GPU adapter"]
fn icons_can_have_their_own_colour() {
  check_icon(1.0, Some(Color::rgb(0.0, 0.0, 1.0)), [0, 0, 255]);
}

#[test]
#[ignore = "needs a GPU adapter"]
fn large_icons_are_drawn_as_images() {
  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  let view: View = Element::new(ElementKind::Box)
    .style(Style {
      text_color: Some(Color::BLACK),
      ..Style::default()
    })
    .children([icons::check().picture().size(300.0).into()])
    .into();
  let (pixels, w, stats) = paint(&gpu, &mut painter, view, (300.0, 300.0), 1.0);
  assert_eq!((stats.glyphs, stats.images), (0, 1));
  // The check's stroke crosses the middle of its short arm.
  let cover = icons::check().render(300, 300);
  let solid = (0..300 * 300)
    .find(|&i| cover.alpha[i] == 255)
    .expect("ink");
  let got = pixels[(solid as u32 / 300 * w + solid as u32 % 300) as usize];
  assert!(got[0] < 5, "{got:?}");
}

#[test]
#[ignore = "needs a GPU adapter"]
fn large_images_upload_over_several_frames() {
  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  // 48 megabytes with its smaller copies: three frames' worth.
  let big = Bitmap::from_rgba(Rgba {
    width: 3000,
    height: 3000,
    pixels: [40, 40, 40, 255].repeat(3000 * 3000),
  });
  let view = || -> View { big.picture().sized(100.0, 100.0).into() };
  let mut frames = 0;
  loop {
    let (p, _, stats) = paint(&gpu, &mut painter, view(), (100.0, 100.0), 1.0);
    frames += 1;
    if stats.waiting.is_none() {
      assert_eq!(stats.images, 1);
      assert_eq!(p[50 * 100 + 50][0], 40, "drawn once uploaded");
      break;
    }
    assert_eq!(stats.images, 0, "left out while uploading");
    assert_eq!(p[50 * 100 + 50], [255, 255, 255, 255]);
    assert!(frames < 10);
  }
  assert!(frames >= 3, "{frames} frames");
}

#[test]
#[ignore = "needs a GPU adapter"]
fn textures_go_with_their_images_and_past_the_budget() {
  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  let a = Bitmap::from_rgba(pattern(64, 64));
  paint(&gpu, &mut painter, a.picture().into(), (64.0, 64.0), 1.0);
  assert_eq!(painter.image_memory().0, 1);
  // Dropped by the application: gone after the next paint.
  drop(a);
  paint(&gpu, &mut painter, View::text("x"), (64.0, 64.0), 1.0);
  assert_eq!(painter.image_memory(), (0, 0));

  // Past the budget, the least recently drawn goes; what is drawn now
  // stays.
  let b = Bitmap::from_rgba(pattern(64, 64));
  let c = Bitmap::from_rgba(pattern(64, 64));
  painter.set_image_budget(b.bytes() + 10);
  paint(&gpu, &mut painter, b.picture().into(), (64.0, 64.0), 1.0);
  paint(&gpu, &mut painter, c.picture().into(), (64.0, 64.0), 1.0);
  assert_eq!(painter.image_memory(), (1, c.bytes()));
}

#[test]
#[ignore = "needs a GPU adapter"]
fn icons_with_several_colours_keep_them() {
  // Red on the left, blue in the middle, and the text's colour on the
  // right (D0024).
  let icon = dovetail_image::Icon::from_svg(
    r##"<svg viewBox="0 0 3 1"><rect width="1" height="1" fill="#f00"/><rect x="1" width="1" height="1" fill="#00f"/><rect x="2" width="1" height="1" fill="currentColor"/></svg>"##,
  )
  .unwrap();
  assert!(icon.is_multicolour());
  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  for (scale, text) in [
    (1.0, [0u8, 128, 0]),
    (2.0, [64, 64, 64]),
  ] {
    let ink = Color::rgb(
      f32::from(text[0]) / 255.0,
      f32::from(text[1]) / 255.0,
      f32::from(text[2]) / 255.0,
    );
    let view: View = Element::new(ElementKind::Box)
      .style(Style {
        text_color: Some(ink),
        ..Style::default()
      })
      .children([icon.picture().sized(30.0, 10.0).into()])
      .into();
    let (p, w, stats) = paint(&gpu, &mut painter, view, (30.0, 10.0), scale);
    assert_eq!(stats.images, 1, "drawn as an image");
    let at =
      |x: f32| p[((5.0 * scale) as u32 * w + (x * scale) as u32) as usize];
    for (x, want) in [
      (5.0, [255, 0, 0]),
      (15.0, [0, 0, 255]),
      (25.0, text),
    ] {
      let got = at(x);
      assert!(
        (0..3).all(|c| got[c].abs_diff(want[c]) <= 2),
        "scale {scale}, x {x}: {got:?}"
      );
    }
  }
}
