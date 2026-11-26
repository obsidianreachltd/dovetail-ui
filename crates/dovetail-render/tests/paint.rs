//! M04 "Done when" item 2 (image tests), and the painting half of item 3
//! (only changed regions are painted again).
//!
//! They paint into a canvas on a real GPU adapter and read the pixels back,
//! so they do not run by default:
//! `cargo test -p dovetail-render --test paint -- --ignored`.

use std::cell::RefCell;
use std::rc::Rc;

use dovetail_core::{
  Border, BuildContext, Callback, Color, Component, Edges, Element,
  ElementKind, Signal, Size, Style, Tree, View,
};
use dovetail_layout::Layout;
use dovetail_render::{Canvas, Gpu, Painter, font_measure};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];

/// Shows the view in its state; tests change the state.
struct Scene;

#[derive(PartialEq)]
struct SceneProps {
  first: View,
  share: Callback<Signal<View>>,
}

impl Component for Scene {
  type Props = SceneProps;

  fn build(
    props: &SceneProps,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let first = props.first.clone();
    let view = cx.use_state(move || first);
    props.share.call(view);
    view.get()
  }
}

struct Fixture {
  gpu: Gpu,
  painter: Painter,
  canvas: Canvas,
  tree: Tree,
  layout: Layout,
  scene: Signal<View>,
}

impl Fixture {
  fn new(view: View) -> Self {
    let gpu = Gpu::headless().expect("a GPU adapter");
    let painter = Painter::new(&gpu);
    let canvas = painter.canvas(&gpu, 100, 100);
    let shared = Rc::new(RefCell::new(None));
    let slot = shared.clone();
    let mut tree = Tree::new();
    tree.mount(View::component::<Scene>(SceneProps {
      first: view,
      share: Callback::new(move |s| *slot.borrow_mut() = Some(s)),
    }));
    let scene = shared.borrow().expect("scene state");
    let mut fixture = Self {
      gpu,
      painter,
      canvas,
      tree,
      layout: Layout::new(),
      scene,
    };
    fixture.paint();
    fixture
  }

  /// Lays out and paints what changed, and returns the pixels painted.
  fn paint(&mut self) -> u64 {
    self.tree.update();
    let changes = self
      .layout
      .update(&self.tree, 100.0, 100.0, &font_measure());
    let stats = self.painter.paint(
      &self.gpu,
      &mut self.canvas,
      &dovetail_render::PaintInput {
        boxes: self.layout.boxes(),
        region: changes
          .damage_bounds()
          .or(Some(dovetail_layout::Rect::default())),
        scale: 1.0,
        background: Color::WHITE,
        focus_ring: None,
        caret_visible: true,
        colors: Default::default(),
        scrollbars: &[],
      },
    );
    stats.pixels()
  }

  fn pixels(&self) -> Vec<[u8; 4]> {
    self.painter.read_pixels(&self.gpu, &self.canvas)
  }

  fn at(
    pixels: &[[u8; 4]],
    x: usize,
    y: usize,
  ) -> [u8; 4] {
    pixels[y * 100 + x]
  }
}

fn stack(children: impl IntoIterator<Item = View>) -> View {
  Element::new(ElementKind::Stack)
    .children(children)
    .into()
}

/// A box at (x, y) of the given size, placed with padding in a stack.
fn placed(
  x: f32,
  y: f32,
  style: Style,
  children: impl IntoIterator<Item = View>,
) -> View {
  let inner: View = Element::new(ElementKind::Box)
    .style(style)
    .children(children)
    .into();
  Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges { left: x, top: y, right: 0.0, bottom: 0.0 },
      ..Style::default()
    })
    .children([inner])
    .into()
}

fn close(
  a: [u8; 4],
  b: [u8; 4],
) -> bool {
  a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2)
}

#[test]
#[ignore = "needs a GPU adapter"]
fn rectangles_borders_and_rounded_corners() {
  let style = Style {
    width: Size::Fixed(60.0),
    height: Size::Fixed(40.0),
    background: Color::rgb(1.0, 0.0, 0.0),
    border: Border {
      width: 4.0,
      color: Color::rgb(0.0, 0.0, 1.0),
    },
    radius: 12.0,
    ..Style::default()
  };
  let f = Fixture::new(stack([placed(10.0, 10.0, style, [])]));
  let p = f.pixels();
  assert!(
    close(Fixture::at(&p, 40, 30), RED),
    "inside is the background: {:?}",
    Fixture::at(&p, 40, 30)
  );
  assert!(
    close(Fixture::at(&p, 11, 30), BLUE),
    "left edge is the border: {:?}",
    Fixture::at(&p, 11, 30)
  );
  assert!(
    close(Fixture::at(&p, 40, 48), BLUE),
    "bottom edge is the border"
  );
  assert!(
    close(Fixture::at(&p, 10, 10), WHITE),
    "the corner is rounded off: {:?}",
    Fixture::at(&p, 10, 10)
  );
  assert!(close(Fixture::at(&p, 69, 49), WHITE), "the far corner too");
  assert!(
    close(Fixture::at(&p, 90, 90), WHITE),
    "outside is the window background"
  );
}

#[test]
#[ignore = "needs a GPU adapter"]
fn clipping_stops_children_drawing_outside() {
  let clipper = Style {
    width: Size::Fixed(50.0),
    height: Size::Fixed(50.0),
    clip: true,
    ..Style::default()
  };
  let big = Style {
    width: Size::Fixed(80.0),
    height: Size::Fixed(80.0),
    background: Color::rgb(0.0, 1.0, 0.0),
    ..Style::default()
  };
  let inner: View = Element::new(ElementKind::Box).style(big).into();
  let f = Fixture::new(stack([Element::new(ElementKind::Box)
    .style(clipper)
    .children([inner])
    .into()]));
  let p = f.pixels();
  assert!(close(Fixture::at(&p, 25, 25), GREEN));
  assert!(close(Fixture::at(&p, 49, 49), GREEN));
  assert!(
    close(Fixture::at(&p, 60, 60), WHITE),
    "clipped: {:?}",
    Fixture::at(&p, 60, 60)
  );
  assert!(
    close(Fixture::at(&p, 55, 10), WHITE),
    "clipped on the right"
  );
}

#[test]
#[ignore = "needs a GPU adapter"]
fn text_is_drawn_inside_its_box() {
  let style = Style {
    font_size: Some(20.0),
    text_color: Some(Color::BLACK),
    ..Style::default()
  };
  let f = Fixture::new(stack([placed(
    10.0,
    10.0,
    style,
    [View::text("H")],
  )]));
  let p = f.pixels();
  // The text box as layout measured it, with the same fonts the painter
  // draws with.
  let text = f
    .layout
    .boxes()
    .iter()
    .find(|b| matches!(b.content, dovetail_layout::Content::Text { .. }))
    .expect("a text box")
    .rect;
  assert_eq!((text.x, text.y), (10.0, 10.0));
  let (xs, ys) = (
    text.x.floor() as usize..(text.x + text.width).ceil() as usize,
    text.y.floor() as usize..(text.y + text.height).ceil() as usize,
  );
  let dark = |x: usize, y: usize| Fixture::at(&p, x, y)[0] < 128;
  let inside = xs
    .clone()
    .flat_map(|x| ys.clone().map(move |y| (x, y)))
    .filter(|&(x, y)| dark(x, y))
    .count();
  let outside = (0..100)
    .flat_map(|x| (0..100).map(move |y| (x, y)))
    .filter(|&(x, y)| !xs.contains(&x) || !ys.contains(&y))
    .filter(|&(x, y)| dark(x, y))
    .count();
  assert!(
    inside > 30,
    "the strokes of H are drawn: {inside} dark pixels"
  );
  assert_eq!(outside, 0, "nothing is drawn outside the text box");
}

#[test]
#[ignore = "needs a GPU adapter"]
fn a_colour_change_repaints_only_its_box() {
  let swatch = |c: Color| Style {
    width: Size::Fixed(20.0),
    height: Size::Fixed(20.0),
    background: c,
    ..Style::default()
  };
  let row = |second: Color| -> View {
    Element::new(ElementKind::Row)
      .children([
        placed(0.0, 0.0, swatch(Color::rgb(1.0, 0.0, 0.0)), []),
        placed(10.0, 0.0, swatch(second), []),
      ])
      .into()
  };
  let mut f = Fixture::new(row(Color::rgb(0.0, 0.0, 1.0)));
  f.scene.set(row(Color::rgb(0.0, 1.0, 0.0)));
  let painted = f.paint();

  let p = f.pixels();
  assert!(
    close(Fixture::at(&p, 40, 10), GREEN),
    "the second swatch changed"
  );
  assert!(
    close(Fixture::at(&p, 10, 10), RED),
    "the first swatch is still there"
  );
  assert_eq!(
    painted,
    20 * 20,
    "only the changed swatch was painted, not 100 × 100"
  );
}

#[test]
#[ignore = "needs a GPU adapter"]
fn a_moved_box_leaves_nothing_behind() {
  let block = Style {
    width: Size::Fixed(20.0),
    height: Size::Fixed(20.0),
    background: Color::rgb(0.0, 0.0, 1.0),
    ..Style::default()
  };
  let mut f = Fixture::new(stack([placed(10.0, 10.0, block.clone(), [])]));
  f.scene
    .set(stack([placed(60.0, 60.0, block, [])]));
  let painted = f.paint();

  let p = f.pixels();
  assert!(
    close(Fixture::at(&p, 70, 70), BLUE),
    "drawn at the new place"
  );
  assert!(
    close(Fixture::at(&p, 20, 20), WHITE),
    "gone from the old place: {:?}",
    Fixture::at(&p, 20, 20)
  );
  assert!(painted < 100 * 100, "a partial repaint: {painted} pixels");
}

#[test]
#[ignore = "needs a GPU adapter"]
fn the_focus_ring_is_drawn_round_the_element() {
  let style = Style {
    width: Size::Fixed(40.0),
    height: Size::Fixed(20.0),
    background: Color::rgb(0.0, 1.0, 0.0),
    ..Style::default()
  };
  let mut f = Fixture::new(stack([placed(20.0, 20.0, style, [])]));
  // The ring of the desktop: 1 pixel gap, 2 pixels wide, outside the box.
  let ring = dovetail_layout::Rect::new(17.0, 17.0, 46.0, 26.0);
  f.painter.paint(
    &f.gpu,
    &mut f.canvas,
    &dovetail_render::PaintInput {
      boxes: f.layout.boxes(),
      region: Some(ring),
      scale: 1.0,
      background: Color::WHITE,
      focus_ring: Some((ring, 3.0)),
      caret_visible: true,
      colors: Default::default(),
      scrollbars: &[],
    },
  );
  let p = f.pixels();
  let ring_colour = [0x33, 0x41, 0x5c, 255];
  assert!(
    close(Fixture::at(&p, 40, 17), ring_colour),
    "top of the ring: {:?}",
    Fixture::at(&p, 40, 17)
  );
  assert!(
    close(Fixture::at(&p, 18, 30), ring_colour),
    "left of the ring"
  );
  assert!(
    close(Fixture::at(&p, 19, 30), WHITE),
    "the gap between ring and box"
  );
  assert!(
    close(Fixture::at(&p, 40, 30), GREEN),
    "the box is not covered"
  );
  assert!(
    close(Fixture::at(&p, 40, 50), WHITE),
    "nothing below the ring"
  );
}

#[test]
#[ignore = "needs a GPU adapter"]
fn the_selection_is_under_the_text_and_the_caret_is_drawn() {
  let style = Style {
    font_size: Some(20.0),
    text_color: Some(Color::rgb(1.0, 0.0, 0.0)),
    // The selection must show over the element's own background.
    background: Color::WHITE,
    ..Style::default()
  };
  let field: View = Element::new(ElementKind::Box)
    .style(style)
    .text_cursor(dovetail_core::TextCursor {
      caret: 2,
      anchor: 0,
      show_caret: true,
      composing: None,
    })
    .children([View::text("HH")])
    .into();
  let f = Fixture::new(stack([placed(
    10.0,
    10.0,
    Style::default(),
    [field],
  )]));
  let p = f.pixels();
  let boxes = f.layout.boxes();
  let text = boxes
    .iter()
    .find(|b| matches!(b.content, dovetail_layout::Content::Text { .. }))
    .expect("a text box");
  let geometry = dovetail_layout::cursor_geometry(
    text,
    &dovetail_core::TextCursor {
      caret: 2,
      anchor: 0,
      show_caret: true,
      composing: None,
    },
    &font_measure(),
  );
  let selection = geometry.selection[0];
  let caret = geometry.caret.expect("a caret");

  // Every pixel of the selected area is the selection colour or red ink.
  let tint = dovetail_render::SELECTION;
  let tinted = [
    (tint.r * 255.0).round() as u8,
    (tint.g * 255.0).round() as u8,
    (tint.b * 255.0).round() as u8,
    255,
  ];
  let mut tinted_count = 0;
  for y in
    selection.y.ceil() as usize..(selection.y + selection.height) as usize
  {
    for x in
      selection.x.ceil() as usize..(selection.x + selection.width) as usize - 1
    {
      let px = Fixture::at(&p, x, y);
      assert_ne!(px, WHITE, "({x}, {y}) is selected but white");
      if close(px, tinted) {
        tinted_count += 1;
      }
    }
  }
  assert!(
    tinted_count > 50,
    "the selection shows: {tinted_count} pixels"
  );

  // The caret is a red line after the last letter, over the full line.
  let x = caret.x.round() as usize;
  let red = (caret.y.ceil() as usize..(caret.y + caret.height) as usize)
    .filter(|&y| close(Fixture::at(&p, x, y), RED))
    .count();
  assert!(
    red as f32 >= caret.height - 2.0,
    "the caret is drawn: {red} red pixels at x = {x}"
  );
}

#[test]
#[ignore = "needs a GPU adapter"]
fn a_scrollbar_is_thin_until_the_pointer_is_over_it() {
  // A 60 × 60 box at (20, 20) with a 200-pixel-tall child: a vertical bar.
  let child: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(40.0),
      height: Size::Fixed(200.0),
      ..Style::default()
    })
    .into();
  let style = Style {
    width: Size::Fixed(60.0),
    height: Size::Fixed(60.0),
    clip: true,
    scroll: true,
    ..Style::default()
  };
  let mut f = Fixture::new(stack([placed(20.0, 20.0, style, [child])]));
  let bars = f.layout.scrollbars();
  assert_eq!(bars.len(), 1);
  let thumb_colour = [255, 0, 0, 255];
  let track_colour = [0, 0, 255, 255];
  let colors = dovetail_core::WindowStyle {
    scrollbar_thumb: Color::rgb(1.0, 0.0, 0.0),
    scrollbar_track: Color::rgb(0.0, 0.0, 1.0),
    ..Default::default()
  };
  let mut paint = |wide: bool| {
    let bars = [(bars[0].clone(), wide)];
    f.painter.paint(
      &f.gpu,
      &mut f.canvas,
      &dovetail_render::PaintInput {
        boxes: f.layout.boxes(),
        region: None,
        scale: 1.0,
        background: Color::WHITE,
        focus_ring: None,
        caret_visible: true,
        colors,
        scrollbars: &bars,
      },
    );
    f.pixels()
  };
  // Thin: 4 pixels wide, 2 in from the right edge (x 74 to 78), at the
  // top, with no track.
  let p = paint(false);
  assert!(close(Fixture::at(&p, 76, 30), thumb_colour), "thin thumb");
  assert!(close(Fixture::at(&p, 71, 30), WHITE), "no track while thin");
  assert!(close(Fixture::at(&p, 76, 70), WHITE), "below the thumb");
  // Wide: the track and an 8-pixel thumb.
  let p = paint(true);
  assert!(close(Fixture::at(&p, 71, 30), thumb_colour), "wide thumb");
  assert!(close(Fixture::at(&p, 71, 70), track_colour), "the track");
  assert!(close(Fixture::at(&p, 60, 30), WHITE), "left of the bar");
}

/// M18: an offset moves a box and what is in it, leaving nothing behind,
/// and the parent's clip still applies.
#[test]
#[ignore = "needs a GPU adapter"]
fn an_offset_moves_a_box_and_its_children() {
  let inner = Style {
    width: Size::Fixed(10.0),
    height: Size::Fixed(10.0),
    background: Color::rgb(1.0, 0.0, 0.0),
    ..Style::default()
  };
  let outer = |offset: (f32, f32)| Style {
    width: Size::Fixed(30.0),
    height: Size::Fixed(30.0),
    padding: Edges::all(10.0),
    background: Color::rgb(0.0, 0.0, 1.0),
    offset,
    ..Style::default()
  };
  let scene = |offset| {
    let red: View = Element::new(ElementKind::Box)
      .style(inner.clone())
      .into();
    let clipper = Style {
      width: Size::Fixed(70.0),
      height: Size::Fixed(70.0),
      clip: true,
      ..Style::default()
    };
    stack([Element::new(ElementKind::Box)
      .style(clipper)
      .children([placed(10.0, 10.0, outer(offset), [red])])
      .into()])
  };
  let mut f = Fixture::new(scene((0.0, 0.0)));
  assert!(close(Fixture::at(&f.pixels(), 25, 25), RED));

  f.scene.set(scene((30.0, 40.0)));
  let painted = f.paint();
  let p = f.pixels();
  assert!(
    close(Fixture::at(&p, 55, 65), RED),
    "the child moved with it"
  );
  assert!(close(Fixture::at(&p, 45, 55), BLUE), "the box moved");
  assert!(
    close(Fixture::at(&p, 25, 25), WHITE),
    "nothing is left behind: {:?}",
    Fixture::at(&p, 25, 25)
  );
  assert!(painted < 100 * 100, "a partial repaint: {painted} pixels");
  // The box would reach 80 down, but its parent clips it at 70.
  assert!(close(Fixture::at(&p, 45, 69), BLUE));
  assert!(
    close(Fixture::at(&p, 45, 72), WHITE),
    "{:?}",
    Fixture::at(&p, 45, 72)
  );
  let moved = f
    .layout
    .boxes()
    .iter()
    .find(|b| matches!(&b.content, dovetail_layout::Content::Element(e) if e.style.offset != (0.0, 0.0)))
    .expect("the moved box");
  assert_eq!((moved.rect.x, moved.rect.y), (40.0, 50.0));
}

/// M18: opacity fades a box, its border and the text in it, multiplied
/// down the tree, and repaints when it changes.
#[test]
#[ignore = "needs a GPU adapter"]
fn opacity_fades_a_box_and_everything_in_it() {
  let scene = |outer: f32, inner: f32| {
    let child: View = Element::new(ElementKind::Box)
      .style(Style {
        width: Size::Fixed(20.0),
        height: Size::Fixed(20.0),
        background: Color::BLACK,
        opacity: inner,
        ..Style::default()
      })
      .into();
    stack([placed(
      10.0,
      10.0,
      Style {
        width: Size::Fixed(60.0),
        height: Size::Fixed(60.0),
        padding: Edges::all(20.0),
        background: Color::rgb(1.0, 0.0, 0.0),
        opacity: outer,
        ..Style::default()
      },
      [child],
    )])
  };
  let mut f = Fixture::new(scene(1.0, 1.0));
  let p = f.pixels();
  assert!(close(Fixture::at(&p, 15, 15), RED));
  assert!(close(Fixture::at(&p, 40, 40), [0, 0, 0, 255]));

  // Half see-through: red over white is pink, mixed in linear light.
  f.scene.set(scene(0.5, 1.0));
  f.paint();
  let p = f.pixels();
  let pink = Fixture::at(&p, 15, 15);
  assert!(close(pink, [255, 188, 188, 255]), "{pink:?}");
  // The child is half see-through too, over the half see-through red.
  let child = Fixture::at(&p, 40, 40);
  assert!(child[0] < pink[0] && child[0] > 50, "{child:?}");
  assert!(child[1] < pink[1], "{child:?}");

  // Opacity multiplies: 0.5 × 0 is clear, so only the parent shows.
  f.scene.set(scene(0.5, 0.0));
  f.paint();
  let p = f.pixels();
  assert!(
    close(Fixture::at(&p, 40, 40), pink),
    "{:?}",
    Fixture::at(&p, 40, 40)
  );

  // At 0 nothing is drawn.
  f.scene.set(scene(0.0, 1.0));
  f.paint();
  let p = f.pixels();
  assert!(close(Fixture::at(&p, 15, 15), WHITE));
  assert!(close(Fixture::at(&p, 40, 40), WHITE));
}
