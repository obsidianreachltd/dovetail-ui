//! Scrollbars (M15, D0021): where layout puts them, dragging them and
//! paging. No GPU is needed.

use dovetail_core::{
  BuildContext, Component, Element, ElementKind, Size, Style, Tree, View,
};
use dovetail_layout::{Content, FixedPitch, Layout, SCROLLBAR_WIDTH};

/// A 200 × 100 scrolling box at the window's top left, around a child of
/// `width` × `height`.
fn scene(
  width: f32,
  height: f32,
  bars: bool,
) -> View {
  let child: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(width),
      height: Size::Fixed(height),
      ..Style::default()
    })
    .into();
  let scroller: View = Element::new(ElementKind::Box)
    .style(Style {
      width: Size::Fixed(200.0),
      height: Size::Fixed(100.0),
      clip: true,
      scroll: true,
      scrollbars: bars,
      ..Style::default()
    })
    .children([child])
    .into();
  Element::new(ElementKind::Box)
    .children([scroller])
    .into()
}

/// Shows the view it is given.
struct Root;

impl Component for Root {
  type Props = View;

  fn build(
    view: &View,
    _: &mut BuildContext<'_>,
  ) -> View {
    view.clone()
  }
}

fn laid_out(view: View) -> (Tree, Layout) {
  let mut tree = Tree::new();
  tree.mount(View::component::<Root>(view));
  tree.update();
  let mut layout = Layout::new();
  layout.update(&tree, 400.0, 300.0, &FixedPitch::default());
  (tree, layout)
}

fn scroller_key(layout: &Layout) -> dovetail_layout::BoxKey {
  layout
    .boxes()
    .iter()
    .find(|b| matches!(&b.content, Content::Element(e) if e.style.scroll))
    .expect("a scrolling element")
    .key
    .clone()
}

#[test]
fn content_that_fits_has_no_bars() {
  let (_tree, layout) = laid_out(scene(150.0, 80.0, true));
  assert!(layout.scrollbars().is_empty());
}

#[test]
fn a_tall_child_gives_a_vertical_bar_along_the_right_edge() {
  let (_tree, layout) = laid_out(scene(150.0, 400.0, true));
  let bars = layout.scrollbars();
  assert_eq!(bars.len(), 1);
  let bar = &bars[0];
  assert!(bar.vertical);
  assert_eq!(bar.track.x, 200.0 - SCROLLBAR_WIDTH);
  assert_eq!(bar.track.height, 100.0);
  // A quarter of the content shows, so the thumb is a quarter of the track.
  assert_eq!(bar.thumb.height, 25.0);
  assert_eq!(bar.thumb.y, 0.0);
  assert_eq!(bar.page, 100.0);
  // It is drawn after the child, the last box inside the element.
  assert_eq!(bar.after, layout.boxes().len() - 1);
}

#[test]
fn both_bars_stop_short_of_the_corner() {
  let (_tree, layout) = laid_out(scene(800.0, 400.0, true));
  let bars = layout.scrollbars();
  assert_eq!(bars.len(), 2);
  let v = bars
    .iter()
    .find(|b| b.vertical)
    .expect("vertical");
  let h = bars.iter().find(|b| !b.vertical).expect("across");
  assert_eq!(v.track.height, 100.0 - SCROLLBAR_WIDTH);
  assert_eq!(h.track.width, 200.0 - SCROLLBAR_WIDTH);
}

#[test]
fn an_element_can_turn_its_bars_off() {
  let (_tree, layout) = laid_out(scene(150.0, 400.0, false));
  assert!(layout.scrollbars().is_empty());
}

#[test]
fn scrolling_moves_the_thumb_and_scroll_to_stays_in_range() {
  let (tree, mut layout) = laid_out(scene(150.0, 400.0, true));
  let key = scroller_key(&layout);
  assert!(layout.scroll_to(&key, None, Some(150.0)));
  layout.update(&tree, 400.0, 300.0, &FixedPitch::default());
  let bar = layout.scrollbars().remove(0);
  // 150 of 300: halfway along the 75 pixels the thumb can move.
  assert_eq!(bar.thumb.y, 37.5);
  // The other way: a thumb start gives the offset.
  assert_eq!(bar.offset_for(37.5, 300.0), 150.0);
  assert_eq!(bar.offset_for(1000.0, 300.0), 300.0);
  // Past the end stops at the end; staying put is not a move.
  assert!(layout.scroll_to(&key, None, Some(5000.0)));
  layout.update(&tree, 400.0, 300.0, &FixedPitch::default());
  assert_eq!(
    layout.scroll_extent(&key),
    Some(((0.0, 300.0), (0.0, 300.0)))
  );
  assert!(!layout.scroll_to(&key, None, Some(300.0)));
}

#[test]
fn the_bar_is_found_under_the_pointer_only_on_its_track() {
  let (_tree, layout) = laid_out(scene(150.0, 400.0, true));
  assert!(layout.scrollbar_at(195.0, 50.0).is_some());
  assert!(layout.scrollbar_at(100.0, 50.0).is_none());
  assert!(layout.scrollbar_at(195.0, 150.0).is_none());
}
