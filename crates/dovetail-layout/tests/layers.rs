//! Layers (M08): elements shown above the window's content, such as menus
//! and command search. No GPU is needed.

use std::cell::Cell;
use std::rc::Rc;

use dovetail_core::{
  BuildContext, Callback, Component, Element, ElementKind, KeyEvent,
  KeyHandler, Layer, NamedKey, Size, Style, Tree, View,
};
use dovetail_layout::{FixedPitch, Layout, Rect};

struct Show;

impl Component for Show {
  type Props = View;

  fn build(
    view: &View,
    _: &mut BuildContext<'_>,
  ) -> View {
    view.clone()
  }
}

fn lay_out(view: View) -> Layout {
  let mut tree = Tree::new();
  tree.mount(View::component::<Show>(view));
  let mut layout = Layout::new();
  layout.update(&tree, 400.0, 300.0, &FixedPitch::default());
  layout
}

fn sized(
  w: f32,
  h: f32,
) -> Style {
  Style {
    width: Size::Fixed(w),
    height: Size::Fixed(h),
    ..Style::default()
  }
}

fn layer(
  at: Layer,
  w: f32,
  h: f32,
) -> Style {
  Style { layer: Some(at), ..sized(w, h) }
}

/// The rectangle of the box with this owner path ending, found by size.
fn rect_sized(
  layout: &Layout,
  w: f32,
  h: f32,
) -> Rect {
  layout
    .boxes()
    .iter()
    .find(|b| b.rect.width == w && b.rect.height == h)
    .map(|b| b.rect)
    .expect("a box of that size")
}

fn el(
  style: Style,
  children: impl IntoIterator<Item = View>,
) -> View {
  Element::new(ElementKind::Box)
    .style(style)
    .children(children)
    .into()
}

fn column(children: impl IntoIterator<Item = View>) -> View {
  Element::new(ElementKind::Column)
    .children(children)
    .into()
}

#[test]
fn a_layer_below_its_parent_is_drawn_last_and_takes_no_room() {
  // A 60 × 20 "title" at (10, 10) with an 80 × 50 menu below it, then a
  // 100 × 30 box after the title.
  let title = el(
    Style {
      padding: dovetail_core::Edges::all(10.0),
      ..Style::default()
    },
    [el(
      sized(60.0, 20.0),
      [el(layer(Layer::Below, 80.0, 50.0), [])],
    )],
  );
  let layout = lay_out(column([title, el(sized(100.0, 30.0), [])]));
  let menu = rect_sized(&layout, 80.0, 50.0);
  assert_eq!((menu.x, menu.y), (10.0, 30.0), "just below the title");
  let after = rect_sized(&layout, 100.0, 30.0);
  assert_eq!(after.y, 40.0, "the menu takes no room in the column");
  // Drawn after everything else, and not clipped by its parent.
  let last = layout.boxes().last().unwrap();
  assert_eq!(last.rect, menu);
  assert_eq!(last.clip, Rect::new(0.0, 0.0, 400.0, 300.0));
  assert_eq!(layout.layers().len(), 1);
}

#[test]
fn layers_stay_inside_the_window() {
  // A title near the bottom right: its menu goes above it, and left.
  let title = el(
    Style {
      padding: dovetail_core::Edges {
        left: 360.0,
        top: 270.0,
        ..Default::default()
      },
      ..Style::default()
    },
    [el(
      sized(30.0, 20.0),
      [el(layer(Layer::Below, 80.0, 50.0), [])],
    )],
  );
  let layout = lay_out(column([title]));
  let menu = rect_sized(&layout, 80.0, 50.0);
  assert_eq!((menu.x, menu.y), (320.0, 220.0));

  let layout = lay_out(column([
    el(layer(Layer::TopCentre(40.0), 200.0, 100.0), []),
    el(layer(Layer::At(390.0, -5.0), 30.0, 30.0), []),
  ]));
  let centred = rect_sized(&layout, 200.0, 100.0);
  assert_eq!((centred.x, centred.y), (100.0, 40.0));
  let at = rect_sized(&layout, 30.0, 30.0);
  assert_eq!((at.x, at.y), (370.0, 0.0));
}

#[test]
fn the_topmost_box_gets_the_pointer() {
  let pressed = Rc::new(Cell::new(""));
  let press = |name: &'static str| {
    let pressed = pressed.clone();
    Callback::new(move |()| pressed.set(name))
  };
  // A button under a backdrop over the whole window, and an item above
  // the backdrop.
  let button: View = Element::new(ElementKind::Box)
    .style(sized(100.0, 40.0))
    .on_press(press("button"))
    .into();
  let backdrop: View = Element::new(ElementKind::Box)
    .style(Style {
      layer: Some(Layer::Fill),
      ..Style::default()
    })
    .on_press(press("backdrop"))
    .children([Element::new(ElementKind::Box)
      .style(layer(Layer::At(200.0, 100.0), 50.0, 20.0))
      .on_press(press("item"))
      .children([View::text("Item")])
      .into()])
    .into();
  let layout = lay_out(column([button, backdrop]));
  layout.hit_test(10.0, 10.0).unwrap().call(());
  assert_eq!(pressed.get(), "backdrop", "the backdrop covers the button");
  // On the item's text: its element is pressed.
  layout.hit_test(205.0, 105.0).unwrap().call(());
  assert_eq!(pressed.get(), "item");
}

#[test]
fn keys_and_tab_inside_layers() {
  let heard = Rc::new(Cell::new(false));
  let ear = heard.clone();
  let field = |n: f32| -> View {
    Element::new(ElementKind::Box)
      .style(sized(n, 10.0))
      .focusable(true)
      .into()
  };
  // A component with a key handler, owning a modal layer with two fields.
  let owner: View = Element::new(ElementKind::Box)
    .on_key(KeyHandler::new(move |e| {
      ear.set(e.is(NamedKey::Escape));
      true
    }))
    .children([Element::new(ElementKind::Column)
      .style(Style {
        layer: Some(Layer::TopCentre(10.0)),
        modal: true,
        ..Style::default()
      })
      .children([field(11.0), field(12.0)])
      .into()])
    .into();
  let layout = lay_out(column([field(10.0), owner]));

  // Tab stays inside the modal layer.
  let order = layout.focus_order();
  assert_eq!(order.len(), 2, "not the field outside it");
  // A key from a field in the layer reaches the element it belongs to.
  let first = order[0].clone();
  assert!(
    layout.dispatch_key(Some(&first), &KeyEvent::named(NamedKey::Escape))
  );
  assert!(heard.get());
}

#[test]
fn with_nothing_focused_keys_go_to_shortcut_scopes() {
  let heard = Rc::new(Cell::new(0));
  let ear = heard.clone();
  let scope: View = Element::new(ElementKind::Box)
    .on_key(KeyHandler::new(move |e| {
      ear.set(ear.get() + 1);
      e.is(NamedKey::F5)
    }))
    .into();
  let layout = lay_out(column([scope]));
  assert!(layout.dispatch_key(None, &KeyEvent::named(NamedKey::F5)));
  assert!(!layout.dispatch_key(None, &KeyEvent::named(NamedKey::Escape)));
  assert_eq!(heard.get(), 2);
  // A focusable element does not get keys without focus.
  let quiet = Rc::new(Cell::new(false));
  let q = quiet.clone();
  let field: View = Element::new(ElementKind::Box)
    .focusable(true)
    .on_key(KeyHandler::new(move |_| {
      q.set(true);
      true
    }))
    .into();
  let layout = lay_out(column([field]));
  assert!(!layout.dispatch_key(None, &KeyEvent::named(NamedKey::F5)));
  assert!(!quiet.get());
}
