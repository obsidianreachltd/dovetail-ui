//! M04 "Done when" item 1 (layout cases) and the layout half of item 3
//! (only changed parts are laid out again). No GPU is needed.

use std::cell::RefCell;
use std::rc::Rc;

use dovetail_core::{
  Align, BuildContext, Callback, Color, Component, Edges, Element, ElementKind,
  Justify, Signal, Size, Style, Tree, View,
};
use dovetail_layout::{Content, FixedPitch, Layout, LayoutBox, Rect};

/// Shows the view it is given.
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

fn el(
  kind: ElementKind,
  style: Style,
  children: impl IntoIterator<Item = View>,
) -> View {
  Element::new(kind)
    .style(style)
    .children(children)
    .into()
}

fn sized(
  width: f32,
  height: f32,
) -> View {
  el(
    ElementKind::Box,
    Style {
      width: Size::Fixed(width),
      height: Size::Fixed(height),
      ..Style::default()
    },
    [],
  )
}

/// Lays `view` out in a window, and returns every box's rectangle, in
/// painting order: parents before children.
fn lay_out(
  view: View,
  width: f32,
  height: f32,
) -> Vec<Rect> {
  let mut tree = Tree::new();
  tree.mount(View::component::<Show>(view));
  let mut layout = Layout::new();
  layout.update(&tree, width, height, &FixedPitch::default());
  layout.boxes().iter().map(|b| b.rect).collect()
}

/// The root is the window; its children start at index 1.
fn root(
  kind: ElementKind,
  style: Style,
  children: impl IntoIterator<Item = View>,
) -> View {
  el(kind, style, children)
}

#[test]
fn row_places_children_side_by_side_with_padding_and_gap() {
  let style = Style {
    padding: Edges::all(10.0),
    gap: 5.0,
    ..Style::default()
  };
  let rects = lay_out(
    root(
      ElementKind::Row,
      style,
      [sized(20.0, 10.0), sized(30.0, 15.0)],
    ),
    200.0,
    100.0,
  );
  assert_eq!(
    rects[0],
    Rect::new(0.0, 0.0, 200.0, 100.0),
    "the root fills the window"
  );
  assert_eq!(rects[1], Rect::new(10.0, 10.0, 20.0, 10.0));
  assert_eq!(rects[2], Rect::new(35.0, 10.0, 30.0, 15.0));
}

#[test]
fn column_justifies_and_aligns_in_the_middle() {
  let style = Style {
    justify: Justify::Center,
    align: Align::Center,
    gap: 10.0,
    ..Style::default()
  };
  let rects = lay_out(
    root(
      ElementKind::Column,
      style,
      [sized(40.0, 20.0), sized(20.0, 20.0)],
    ),
    100.0,
    100.0,
  );
  // Used height 20 + 10 + 20 = 50, so the first starts at (100 - 50) / 2.
  assert_eq!(rects[1], Rect::new(30.0, 25.0, 40.0, 20.0));
  assert_eq!(rects[2], Rect::new(40.0, 55.0, 20.0, 20.0));
}

#[test]
fn space_between_puts_the_last_child_at_the_end() {
  let style = Style {
    justify: Justify::SpaceBetween,
    ..Style::default()
  };
  let rects = lay_out(
    root(
      ElementKind::Row,
      style,
      [
        sized(10.0, 10.0),
        sized(10.0, 10.0),
        sized(10.0, 10.0),
      ],
    ),
    100.0,
    50.0,
  );
  assert_eq!([rects[1].x, rects[2].x, rects[3].x], [0.0, 45.0, 90.0]);
}

#[test]
fn stack_places_children_on_top_of_each_other_at_the_end() {
  let style = Style { align: Align::End, ..Style::default() };
  let rects = lay_out(
    root(
      ElementKind::Stack,
      style,
      [sized(50.0, 50.0), sized(10.0, 20.0)],
    ),
    100.0,
    80.0,
  );
  assert_eq!(rects[1], Rect::new(50.0, 30.0, 50.0, 50.0));
  assert_eq!(rects[2], Rect::new(90.0, 60.0, 10.0, 20.0));
}

#[test]
fn growing_children_share_the_space_left() {
  let grow = |g: f32| {
    el(
      ElementKind::Box,
      Style {
        grow: g,
        height: Size::Fixed(10.0),
        ..Style::default()
      },
      [],
    )
  };
  let rects = lay_out(
    root(
      ElementKind::Row,
      Style::default(),
      [sized(20.0, 10.0), grow(1.0), grow(3.0)],
    ),
    100.0,
    10.0,
  );
  assert_eq!(
    rects[2],
    Rect::new(20.0, 0.0, 20.0, 10.0),
    "a quarter of the 80 left"
  );
  assert_eq!(rects[3], Rect::new(40.0, 0.0, 60.0, 10.0), "three quarters");
}

#[test]
fn stretch_makes_children_as_wide_as_a_column() {
  let style = Style {
    align: Align::Stretch,
    padding: Edges::xy(5.0, 0.0),
    ..Style::default()
  };
  let child = el(
    ElementKind::Box,
    Style {
      height: Size::Fixed(10.0),
      ..Style::default()
    },
    [],
  );
  let rects = lay_out(root(ElementKind::Column, style, [child]), 100.0, 50.0);
  assert_eq!(rects[1], Rect::new(5.0, 0.0, 90.0, 10.0));
}

#[test]
fn a_column_that_fits_its_content_stretches_children_to_its_widest() {
  // As a menu: a column with a smallest width, its rows as wide as it.
  let menu = el(
    ElementKind::Column,
    Style {
      align: Align::Stretch,
      min_width: 80.0,
      ..Style::default()
    },
    [
      sized(30.0, 10.0),
      el(ElementKind::Box, Style::default(), [sized(50.0, 10.0)]),
    ],
  );
  let rects = lay_out(
    root(ElementKind::Column, Style::default(), [menu]),
    300.0,
    200.0,
  );
  assert_eq!(rects[1].width, 80.0, "the menu keeps its smallest width");
  assert_eq!(
    rects[3].width, 80.0,
    "a row that fits its content is stretched"
  );
  assert_eq!(rects[4].width, 50.0, "what is inside the row is not");
  assert_eq!(rects[2].width, 80.0, "as in a column of known width");
}

#[test]
fn minimum_and_maximum_sizes_are_kept() {
  let at_most = Style { max_width: 30.0, ..Style::default() };
  let at_least = Style {
    min_width: 50.0,
    min_height: 40.0,
    ..Style::default()
  };
  let wide_text = el(
    ElementKind::Box,
    at_most,
    [View::text("much too long for thirty")],
  );
  let small = el(ElementKind::Box, at_least, [sized(10.0, 10.0)]);
  let rects = lay_out(
    root(ElementKind::Column, Style::default(), [wide_text, small]),
    200.0,
    200.0,
  );
  assert_eq!(rects[1].width, 30.0, "maximum width");
  assert_eq!(
    (rects[3].width, rects[3].height),
    (50.0, 40.0),
    "minimum size"
  );
}

#[test]
fn text_is_measured_with_the_inherited_font_size() {
  let style = Style {
    font_size: Some(10.0),
    padding: Edges::all(2.0),
    ..Style::default()
  };
  let label = el(ElementKind::Box, style, [View::text("abcd")]);
  let rects = lay_out(
    root(ElementKind::Column, Style::default(), [label]),
    200.0,
    200.0,
  );
  // 4 characters × 0.6 × 10 wide, 1.25 × 10 high (FixedPitch, D0012).
  assert_eq!(rects[2], Rect::new(2.0, 2.0, 24.0, 12.5));
  assert_eq!(rects[1], Rect::new(0.0, 0.0, 28.0, 16.5));
}

#[test]
fn clipping_limits_where_children_may_draw() {
  let clip = Style {
    width: Size::Fixed(50.0),
    height: Size::Fixed(50.0),
    clip: true,
    ..Style::default()
  };
  let mut tree = Tree::new();
  tree.mount(View::component::<Show>(root(
    ElementKind::Stack,
    Style::default(),
    [el(
      ElementKind::Box,
      clip,
      [sized(80.0, 80.0)],
    )],
  )));
  let mut layout = Layout::new();
  layout.update(&tree, 200.0, 200.0, &FixedPitch::default());
  let inner = &layout.boxes()[2];
  assert_eq!(inner.rect, Rect::new(0.0, 0.0, 80.0, 80.0));
  assert_eq!(inner.clip, Rect::new(0.0, 0.0, 50.0, 50.0));
}

// --- Only changed parts are laid out again (item 3) ------------------------

/// A list of items. The test changes the items' texts and colours.
struct List;

#[derive(PartialEq)]
struct ListProps {
  share: Callback<Signal<Vec<(String, u32)>>>,
}

impl Component for List {
  type Props = ListProps;

  fn build(
    props: &ListProps,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let items = cx.use_state(|| {
      (0..100)
        .map(|i| (format!("item {i}"), 0xffffff))
        .collect::<Vec<_>>()
    });
    props.share.call(items);
    let rows = items.get().into_iter().map(|(text, colour)| {
      let style = Style {
        background: Color::hex(colour),
        padding: Edges::all(2.0),
        ..Style::default()
      };
      el(ElementKind::Box, style, [View::text(text)])
    });
    el(
      ElementKind::Column,
      Style { gap: 1.0, ..Style::default() },
      rows,
    )
  }
}

fn mount_list() -> (Tree, Signal<Vec<(String, u32)>>, Layout) {
  let shared = Rc::new(RefCell::new(None));
  let slot = shared.clone();
  let mut tree = Tree::new();
  tree.mount(View::component::<List>(ListProps {
    share: Callback::new(move |s| *slot.borrow_mut() = Some(s)),
  }));
  let mut layout = Layout::new();
  let first = layout.update(&tree, 400.0, 3000.0, &FixedPitch::default());
  assert_eq!(
    first.stats.laid_out, first.stats.boxes,
    "the first update lays out everything"
  );
  let items = shared.borrow().expect("items");
  (tree, items, layout)
}

fn rects(layout: &Layout) -> Vec<Rect> {
  layout
    .boxes()
    .iter()
    .map(|b: &LayoutBox| b.rect)
    .collect()
}

/// Lays out the tree from nothing, to compare with an incremental result.
fn full_layout(tree: &Tree) -> Vec<Rect> {
  let mut fresh = Layout::new();
  fresh.update(tree, 400.0, 3000.0, &FixedPitch::default());
  rects(&fresh)
}

#[test]
fn changing_one_text_lays_out_only_that_item_and_its_parents() {
  let (mut tree, items, mut layout) = mount_list();
  items.update(|items| items[50].0 = "a much longer item fifty".to_owned());
  tree.update();
  let changes = layout.update(&tree, 400.0, 3000.0, &FixedPitch::default());

  // The column is the root, then a box and a text for each item. Only the
  // changed text, its box and the column are laid out again; the other 99
  // items are reused.
  assert_eq!(changes.stats.boxes, 201);
  assert_eq!(changes.stats.laid_out, 3);
  assert_eq!(changes.stats.reused, 198);
  assert_eq!(
    rects(&layout),
    full_layout(&tree),
    "incremental equals full layout"
  );
  let damage = changes.damage_bounds().expect("damage");
  assert!(
    damage.width < 400.0 && damage.height < 30.0,
    "only item fifty is damaged: {damage:?}"
  );
}

#[test]
fn changing_a_colour_lays_out_nothing_and_damages_one_box() {
  let (mut tree, items, mut layout) = mount_list();
  items.update(|items| items[10].1 = 0xff0000);
  tree.update();
  let changes = layout.update(&tree, 400.0, 3000.0, &FixedPitch::default());

  assert_eq!(changes.stats.laid_out, 0);
  let item = layout.boxes()[1 + 2 * 10].rect;
  assert_eq!(changes.damage, [item, item]);
}

#[test]
fn removing_an_item_damages_where_it_was_and_where_the_rest_moved() {
  let (mut tree, items, mut layout) = mount_list();
  let first_text = layout.boxes()[2].rect;
  let last_before = layout.boxes()[1 + 2 * 99].rect;
  items.update(|items| {
    items.remove(0);
  });
  tree.update();
  let changes = layout.update(&tree, 400.0, 3000.0, &FixedPitch::default());

  assert_eq!(rects(&layout), full_layout(&tree));
  // Boxes are matched by position, so every text now shows the next item's
  // text, and the last box is gone. The item boxes keep their place and
  // look, so they are not damaged.
  let damage = changes.damage_bounds().expect("damage");
  assert_eq!(damage.y, first_text.y);
  assert_eq!(damage.y + damage.height, last_before.y + last_before.height);
}

#[test]
fn hit_testing_finds_the_deepest_press_handler() {
  let pressed = Rc::new(RefCell::new(Vec::new()));
  let handler = |name: &'static str| {
    let pressed = pressed.clone();
    Callback::new(move |()| pressed.borrow_mut().push(name))
  };
  let inner = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(20.0),
      height: Size::Fixed(20.0),
      ..Style::default()
    })
    .on_press(handler("inner"));
  let outer = Element::new(ElementKind::Box)
    .style(Style {
      padding: Edges::all(10.0),
      ..Style::default()
    })
    .on_press(handler("outer"))
    .children([inner.into()]);
  let mut tree = Tree::new();
  tree.mount(View::component::<Show>(root(
    ElementKind::Stack,
    Style::default(),
    [outer.into()],
  )));
  let mut layout = Layout::new();
  layout.update(&tree, 100.0, 100.0, &FixedPitch::default());

  layout
    .hit_test(15.0, 15.0)
    .expect("inner")
    .call(());
  layout.hit_test(5.0, 5.0).expect("outer").call(());
  assert!(layout.hit_test(90.0, 90.0).is_none());
  assert_eq!(*pressed.borrow(), ["inner", "outer"]);
  assert!(matches!(layout.boxes()[2].content, Content::Element(_)));
}
