//! Scrolling elements (`Style::scroll`): the wheel, limits, and keeping a
//! shown caret in view (M06). No GPU is needed.

use std::cell::Cell;

use dovetail_core::{
  BuildContext, Callback, Component, Edges, Element, ElementKind, Signal, Size,
  Style, TextCursor, Tree, View,
};
use dovetail_layout::{Content, FixedPitch, Layout};

thread_local! {
  static SCENE: Cell<Option<Signal<View>>> = const { Cell::new(None) };
}

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

struct Window {
  tree: Tree,
  layout: Layout,
}

impl Window {
  fn new(view: View) -> Self {
    let mut tree = Tree::new();
    tree.mount(View::component::<Scene>(SceneProps {
      first: wrap(view),
      share: Callback::new(|s| SCENE.set(Some(s))),
    }));
    let mut w = Window { tree, layout: Layout::new() };
    w.update();
    w
  }

  fn show(
    &mut self,
    view: View,
  ) {
    SCENE.get().expect("scene").set(wrap(view));
    self.update();
  }

  fn update(&mut self) {
    self.tree.update();
    self
      .layout
      .update(&self.tree, 400.0, 300.0, &FixedPitch::default());
  }

  /// The x of the first text box, or of the first box inside the
  /// scrolling one.
  fn inner_x(&self) -> f32 {
    let boxes = self.layout.boxes();
    let outer = boxes
      .iter()
      .position(|b| matches!(&b.content, Content::Element(e) if e.style.scroll))
      .expect("a scrolling element");
    boxes[boxes[outer].children[0]].rect.x
  }
}

/// The window's root fills the window; the view goes inside it.
fn wrap(view: View) -> View {
  Element::new(ElementKind::Box)
    .children([view])
    .into()
}

/// A 100 × 28 scrolling box, with padding 4, around `child`.
fn scroller(
  child: View,
  cursor: Option<TextCursor>,
) -> View {
  let mut e = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(100.0),
      height: Size::Fixed(28.0),
      padding: Edges::all(4.0),
      clip: true,
      scroll: true,
      ..Style::default()
    })
    .children([child]);
  if let Some(cursor) = cursor {
    e = e.text_cursor(cursor);
  }
  e.into()
}

fn wide() -> View {
  Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(300.0),
      height: Size::Fixed(20.0),
      ..Style::default()
    })
    .into()
}

/// 30 characters: 252 pixels wide at the default size, 14, × 0.6.
const TEXT: &str = "abcdefghijklmnopqrstuvwxyz0123";

fn caret(
  at: usize,
  shown: bool,
) -> Option<TextCursor> {
  Some(TextCursor {
    caret: at,
    anchor: at,
    show_caret: shown,
    composing: None,
  })
}

#[test]
fn the_wheel_scrolls_within_the_content() {
  let mut w = Window::new(scroller(wide(), None));
  assert_eq!(w.inner_x(), 4.0);
  assert!(w.layout.scroll_by(50.0, 10.0, 40.0, 0.0));
  w.update();
  assert_eq!(w.inner_x(), 4.0 - 40.0);
  // The content is 300 wide in a 92-wide space: 208 at most.
  assert!(w.layout.scroll_by(50.0, 10.0, 1000.0, 0.0));
  w.update();
  assert_eq!(w.inner_x(), 4.0 - 208.0);
  assert!(
    !w.layout.scroll_by(50.0, 10.0, 10.0, 0.0),
    "already at the end"
  );
  assert!(
    !w.layout.scroll_by(50.0, 10.0, 0.0, 10.0),
    "nothing to scroll down"
  );
  assert!(!w.layout.scroll_by(300.0, 200.0, -10.0, 0.0), "not over it");
}

#[test]
fn a_shown_caret_is_kept_in_view() {
  let advance = 0.6 * dovetail_layout::DEFAULT_FONT_SIZE;
  // The caret at the end: the text moves left until the caret shows.
  let mut w = Window::new(scroller(View::text(TEXT), caret(30, true)));
  let caret_x = w.inner_x() + 30.0 * advance;
  assert!(
    caret_x + 1.0 <= 96.0 + 0.01 && caret_x >= 4.0,
    "the caret, at {caret_x}, is inside the content box"
  );
  // Back to the start: the text moves back.
  w.show(scroller(View::text(TEXT), caret(0, true)));
  assert_eq!(w.inner_x(), 4.0);
  // A caret that is not shown does not scroll.
  w.show(scroller(View::text(TEXT), caret(30, false)));
  assert_eq!(w.inner_x(), 4.0);
}

#[test]
fn the_wheel_is_not_undone_until_the_caret_moves() {
  let mut w = Window::new(scroller(View::text(TEXT), caret(0, true)));
  assert!(w.layout.scroll_by(50.0, 10.0, 50.0, 0.0));
  w.update();
  assert_eq!(w.inner_x(), 4.0 - 50.0, "the caret is now out of view");
  w.update();
  assert_eq!(w.inner_x(), 4.0 - 50.0, "and stays so");
  w.show(scroller(View::text(TEXT), caret(1, true)));
  assert!(w.inner_x() >= 4.0 - 9.6, "moving the caret brings it back");
}

#[test]
fn the_wheel_passes_out_when_the_inner_element_is_at_its_end() {
  let inner = scroller(wide(), None);
  let outer: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(200.0),
      height: Size::Fixed(28.0),
      clip: true,
      scroll: true,
      ..Style::default()
    })
    .children([Element::new(ElementKind::Row)
      .children([inner, wide()])
      .into()])
    .into();
  let mut w = Window::new(outer);
  // The inner one goes first.
  assert!(w.layout.scroll_by(50.0, 10.0, 1000.0, 0.0));
  w.update();
  // The root is box 0, and the outer scrolling box is box 1.
  let outer_key = w.layout.boxes()[1].key.clone();
  assert_eq!(w.layout.scroll_offset(&outer_key), (0.0, 0.0));
  // At its end, the outer one scrolls.
  assert!(w.layout.scroll_by(50.0, 10.0, 30.0, 0.0));
  w.update();
  assert_eq!(w.layout.scroll_offset(&outer_key), (30.0, 0.0));
}

#[test]
fn an_element_that_takes_the_wheel_is_found_before_scrolling() {
  use dovetail_core::WheelEvent;
  let canvas: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(300.0),
      height: Size::Fixed(20.0),
      ..Style::default()
    })
    .on_wheel(Callback::new(|_: WheelEvent| {}))
    .into();
  let w = Window::new(scroller(canvas, None));
  let target = w
    .layout
    .wheel_target_at(50.0, 10.0)
    .expect("the canvas");
  let element = w.layout.element(&target).unwrap();
  assert!(element.on_wheel.is_some());
  assert!(
    w.layout.wheel_target_at(390.0, 290.0).is_none(),
    "not elsewhere"
  );
}
