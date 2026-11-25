//! Paints a sample scene and saves it as a PNG, to look at the painter's
//! output, the font in particular. Run with
//! `cargo run -p dovetail-render --example snapshot -- out.png [scale]`.

use dovetail_core::{
  Align, Border, BuildContext, Color, Component, Edges, Element, ElementKind,
  Size, Style, TextCursor, Tree, View,
};
use dovetail_layout::Layout;
use dovetail_render::{Gpu, PaintInput, Painter, font_measure};

struct Sample;

impl Component for Sample {
  type Props = ();

  fn build(
    _: &(),
    _: &mut BuildContext<'_>,
  ) -> View {
    let text = |t: &str, size: f32| -> View {
      Element::new(ElementKind::Box)
        .style(Style {
          font_size: Some(size),
          ..Style::default()
        })
        .children([View::text(t)])
        .into()
    };
    let button = Element::new(ElementKind::Box)
      .style(Style {
        padding: Edges::xy(12.0, 6.0),
        background: Color::hex(0x1e66f5),
        radius: 8.0,
        text_color: Some(Color::WHITE),
        ..Style::default()
      })
      .children([View::text("Increment")]);
    // A text field with "world" selected and the caret after it, and text
    // being composed underlined.
    let field = Element::new(ElementKind::Box)
      .style(Style {
        padding: Edges::xy(8.0, 5.0),
        width: Size::Fixed(220.0),
        background: Color::WHITE,
        border: Border { width: 1.0, color: Color::hex(0x1e66f5) },
        radius: 4.0,
        clip: true,
        ..Style::default()
      })
      .text_cursor(TextCursor {
        caret: 11,
        anchor: 6,
        show_caret: true,
        // "second", as if an input method were composing it.
        composing: Some((14, 20)),
      })
      .children([View::text(
        "Hello world
A second line",
      )]);
    let card = Element::new(ElementKind::Column)
      .style(Style {
        padding: Edges::all(16.0),
        gap: 10.0,
        background: Color::WHITE,
        border: Border { width: 2.0, color: Color::hex(0x9ca0b0) },
        radius: 12.0,
        align: Align::Start,
        ..Style::default()
      })
      .children([
        text("Count: 42", 24.0),
        button.into(),
        field.into(),
        text("Kerning: AVAWA Today Yes LT Wave", 24.0),
        text("ABCDEFGHIJKLMNOPQRSTUVWXYZ", 14.0),
        text("abcdefghijklmnopqrstuvwxyz", 14.0),
        text("0123456789 !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~", 14.0),
        text("Fallback: 中文 日本語 한국어 ✓ €", 16.0),
        // M13: Arabic with vowel marks, and Arabic in an English line.
        text("\u{0628}\u{0650}\u{0633}\u{0652}\u{0645}\u{0650} \u{0627}\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{0650} \u{0627}\u{0644}\u{0633}\u{0651}\u{064E}\u{0644}\u{0627}\u{0645} \u{0639}\u{0644}\u{064A}\u{0643}\u{0645}", 24.0),
        text("Mixed: The word \u{0633}\u{0644}\u{0627}\u{0645} (salam) means peace, 123.", 18.0),
      ]);
    Element::new(ElementKind::Box)
      .style(Style {
        padding: Edges::all(20.0),
        width: Size::Fill,
        height: Size::Fill,
        background: Color::hex(0xeff1f5),
        ..Style::default()
      })
      .children([card.into()])
      .into()
  }
}

fn main() {
  let mut args = std::env::args().skip(1);
  let path = args
    .next()
    .unwrap_or_else(|| "snapshot.png".to_owned());
  let scale: f32 = args
    .next()
    .and_then(|s| s.parse().ok())
    .unwrap_or(1.0);
  let (width, height) = (560.0, 420.0);

  let gpu = Gpu::headless().expect("a GPU adapter");
  let mut painter = Painter::new(&gpu);
  let mut canvas =
    painter.canvas(&gpu, (width * scale) as u32, (height * scale) as u32);
  let mut tree = Tree::new();
  tree.mount(View::component::<Sample>(()));
  let mut layout = Layout::new();
  layout.update(&tree, width, height, &font_measure());
  painter.paint(&gpu, &mut canvas, &PaintInput::new(layout.boxes(), scale));
  let pixels = painter.read_pixels(&gpu, &canvas);
  let png = dovetail_image::encode_png(&dovetail_image::Rgba {
    width: canvas.width(),
    height: canvas.height(),
    pixels: pixels.into_iter().flatten().collect(),
  });
  std::fs::write(&path, png).expect("write the PNG");
  println!("wrote {path} ({} x {})", canvas.width(), canvas.height());
}
