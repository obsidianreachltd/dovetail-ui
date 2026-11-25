//! The board: a canvas with pan, zoom and selection, and a line saying
//! what is selected.

use std::rc::Rc;

use dovetail::controls::{Camera, SelectionBox};
use dovetail::prelude::*;
use dovetail::{
  Edges, Element, ElementKind, KeyCode, KeyHandler, NamedKey, PointerButton,
  PointerHandler, PointerKind, Role, Size, Style,
};

use crate::render::{BoardPainter, ShapeRenderer};
use crate::scene::{self, Scene};

/// Space round the board when it is fitted to the canvas, in pixels.
pub const MARGIN: f32 = 16.0;
/// How far the pointer may move and still count as a click, in pixels.
const SLOP: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
  /// Selecting: where the main button went down and where it is now, in
  /// canvas pixels, and whether to add to the selection.
  Select { area: SelectionBox, adding: bool },
  /// Panning with the middle or right button: where the pointer was.
  Pan { last: (f32, f32) },
}

/// The text for what is selected.
pub fn selected_text(
  scene: &Scene,
  selected: &[bool],
) -> String {
  let chosen: Vec<&str> = selected
    .iter()
    .enumerate()
    .filter(|(_, s)| **s)
    .map(|(i, _)| scene.shapes[i].name.as_str())
    .collect();
  match chosen.as_slice() {
    [] => "Selected: none".to_owned(),
    [one] => format!("Selected: {one}"),
    many => format!("Selected: {} shapes", many.len()),
  }
}

/// The camera a board starts with: the whole board, centred.
pub fn first_camera(
  scene: &Scene,
  size: (f32, f32),
) -> Camera {
  Camera::fit(scene.board, size, MARGIN)
}

/// A board in a canvas of `width` × `height` logical pixels: the small
/// board, or with `large` the one with 10,000 shapes.
///
/// - Click a shape to select it; Shift+click adds or removes it.
/// - Drag a box with the main button to select what it touches; with
///   Shift, to add.
/// - Drag with the middle or right button, or move two fingers on a
///   trackpad, to pan; the arrow keys pan too.
/// - Turn the wheel, or pinch, to zoom about the pointer; + and - zoom
///   about the middle, and 0 shows the whole board.
/// - Ctrl+A (Command+A) selects everything; Escape selects nothing.
#[component]
pub fn Board(
  #[prop(default)] large: bool,
  #[prop(default = 760.0)] width: f32,
  #[prop(default = 470.0)] height: f32,
) -> View {
  let renderer = use_state(|| Rc::new(ShapeRenderer::default())).get();
  let scene = use_state(move || {
    Rc::new(if large {
      scene::large()
    } else {
      scene::small()
    })
  })
  .get();
  let fitted = first_camera(&scene, (width, height));
  let camera = use_state(move || fitted);
  let count = scene.shapes.len();
  let selected = use_state(move || Rc::new(vec![false; count]));
  let drag = use_state(|| None::<Drag>);

  let select = {
    let scene = scene.clone();
    move |hits: Vec<usize>, adding: bool, toggle: bool| {
      let mut next = if adding {
        (*selected.get()).clone()
      } else {
        vec![false; scene.shapes.len()]
      };
      for i in hits {
        next[i] = if toggle { !next[i] } else { true };
      }
      selected.set(Rc::new(next));
    }
  };

  let pointer = {
    let scene = scene.clone();
    let select = select.clone();
    PointerHandler::new(move |e| {
      let at = (e.x, e.y);
      match (e.kind, e.button) {
        (PointerKind::Down, PointerButton::Primary) => {
          drag.set(Some(Drag::Select {
            area: SelectionBox { start: at, end: at },
            adding: e.modifiers.shift,
          }))
        },
        (PointerKind::Down, _) => drag.set(Some(Drag::Pan { last: at })),
        (PointerKind::Move, _) => match drag.get() {
          Some(Drag::Select { area, adding }) => drag.set(Some(Drag::Select {
            area: SelectionBox { start: area.start, end: at },
            adding,
          })),
          Some(Drag::Pan { last }) => {
            camera.update(|c| *c = c.pan((at.0 - last.0, at.1 - last.1)));
            drag.set(Some(Drag::Pan { last: at }));
          },
          None => {},
        },
        (PointerKind::Up, _) => {
          if let Some(Drag::Select { area, adding }) = drag.get() {
            let area = SelectionBox { start: area.start, end: at };
            let c = camera.get();
            if area.is_click(SLOP) {
              let hit = scene.shape_at(c.to_world(at), SLOP / c.zoom);
              select(hit.into_iter().collect(), adding, adding);
            } else {
              let (x0, y0, x1, y1) = area.rect();
              let a = c.to_world((x0, y0));
              let b = c.to_world((x1, y1));
              select(scene.shapes_in((a.0, a.1, b.0, b.1)), adding, false);
            }
          }
          drag.set(None);
        },
      }
    })
  };

  let wheel = Callback::new(move |e: dovetail::WheelEvent| {
    camera.update(|c| {
      *c = if e.steps || e.modifiers.control || e.modifiers.meta {
        c.zoom_at((e.x, e.y), Camera::wheel_factor(e.dy))
      } else {
        c.pan((-e.dx, -e.dy))
      };
    });
  });

  let keys = {
    let select = select.clone();
    let count = scene.shapes.len();
    KeyHandler::new(move |e| {
      let middle = (width / 2.0, height / 2.0);
      let step = 40.0;
      match &e.key {
        KeyCode::Named(NamedKey::ArrowLeft) => {
          camera.update(|c| *c = c.pan((step, 0.0)))
        },
        KeyCode::Named(NamedKey::ArrowRight) => {
          camera.update(|c| *c = c.pan((-step, 0.0)))
        },
        KeyCode::Named(NamedKey::ArrowUp) => {
          camera.update(|c| *c = c.pan((0.0, step)))
        },
        KeyCode::Named(NamedKey::ArrowDown) => {
          camera.update(|c| *c = c.pan((0.0, -step)))
        },
        KeyCode::Named(NamedKey::Escape) => {
          drag.set(None);
          select(Vec::new(), false, false);
        },
        KeyCode::Character(c)
          if e.modifiers.command() && c.eq_ignore_ascii_case("a") =>
        {
          select((0..count).collect(), false, false)
        },
        KeyCode::Character(c) if c == "+" || c == "=" => {
          camera.update(|cam| *cam = cam.zoom_at(middle, 1.25))
        },
        KeyCode::Character(c) if c == "-" => {
          camera.update(|cam| *cam = cam.zoom_at(middle, 0.8))
        },
        KeyCode::Character(c) if c == "0" => camera.set(fitted),
        _ => return false,
      }
      true
    })
  };

  let c = camera.get();
  let chosen = selected.get();
  let selecting = match drag.get() {
    Some(Drag::Select { area, .. }) if !area.is_click(SLOP) => {
      let (x0, y0, x1, y1) = area.rect();
      let a = c.to_world((x0, y0));
      let b = c.to_world((x1, y1));
      Some((a.0, a.1, b.0, b.1))
    },
    _ => None,
  };
  let status = selected_text(&scene, &chosen);
  let painter = BoardPainter {
    renderer,
    scene: scene.clone(),
    selected: chosen.clone(),
    camera: c,
    selecting,
  };
  let board: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(width),
      height: Size::Fixed(height),
      background: Color::hex(0x10141a),
      radius: 6.0,
      clip: true,
      ..Style::default()
    })
    .role(Role::Canvas)
    .label(format!(
      "Board: {} shapes, {}",
      scene.shapes.len(),
      status.to_lowercase()
    ))
    .focusable(true)
    .on_pointer(pointer)
    .on_wheel(wheel)
    .on_key(keys)
    .canvas(dovetail::render::canvas(painter))
    .into();
  let zoom = format!("Zoom {:.0}%", c.zoom / fitted.zoom * 100.0);
  Element::new(ElementKind::Column)
    .style(Style {
      gap: 8.0,
      padding: Edges::all(0.0),
      ..Style::default()
    })
    .children([
      board,
      ui! {
        Row {
          gap: 24.0,
          Text { value: zoom }
          Text { value: status }
        }
      },
    ])
    .into()
}
