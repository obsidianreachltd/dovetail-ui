//! M10: finding drop targets under the pointer, and where a focus target
//! sends focus, with no window or GPU.

use dovetail_core::{
  BuildContext, Component, DropHandler, Element, ElementKind, FocusTarget,
  Size, Style, Tree, View,
};
use dovetail_layout::{FixedPitch, Layout};

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

fn sized(
  width: f32,
  height: f32,
) -> Style {
  Style {
    width: Size::Fixed(width),
    height: Size::Fixed(height),
    ..Style::default()
  }
}

fn laid_out(root: View) -> (Tree, Layout) {
  let mut tree = Tree::new();
  tree.mount(View::component::<Show>(root));
  let mut layout = Layout::new();
  layout.update(&tree, 400.0, 400.0, &FixedPitch::default());
  (tree, layout)
}

#[test]
fn drop_targets_are_listed_nearest_first() {
  let inner: View = Element::new(ElementKind::Box)
    .style(sized(50.0, 50.0))
    .on_drop(DropHandler::new(|_| true))
    .label("inner")
    .into();
  let middle: View = Element::new(ElementKind::Box)
    .style(sized(100.0, 100.0))
    .children([inner])
    .into();
  let outer: View = Element::new(ElementKind::Box)
    .style(sized(200.0, 200.0))
    .on_drop(DropHandler::new(|_| true))
    .label("outer")
    .children([middle])
    .into();
  // The root fills the window, so the outer box goes inside a plain one.
  let root: View = Element::new(ElementKind::Box)
    .children([outer])
    .into();
  let (_tree, layout) = laid_out(root);
  let label = |k: &dovetail_layout::BoxKey| {
    layout
      .element(k)
      .and_then(|e| e.label.clone())
      .unwrap_or_default()
  };

  let at = layout.drop_targets_at(10.0, 10.0);
  assert_eq!(at.iter().map(label).collect::<Vec<_>>(), ["inner", "outer"]);
  // Outside the inner box, only the outer one.
  let at = layout.drop_targets_at(150.0, 150.0);
  assert_eq!(at.iter().map(label).collect::<Vec<_>>(), ["outer"]);
  assert!(layout.drop_targets_at(300.0, 300.0).is_empty());
}

#[test]
fn a_focus_target_sends_focus_to_the_first_focusable_inside() {
  let panel = FocusTarget::named("panel");
  let empty = FocusTarget::named("empty");
  let field = |name: &str| -> View {
    Element::new(ElementKind::Box)
      .style(sized(40.0, 20.0))
      .focusable(true)
      .label(name)
      .into()
  };
  let target: View = Element::new(ElementKind::Column)
    .focus_target(panel)
    .children([field("first"), field("second")])
    .into();
  let lone: View = Element::new(ElementKind::Box)
    .style(sized(40.0, 20.0))
    .focus_target(empty)
    .into();
  let root: View = Element::new(ElementKind::Column)
    .children([field("before"), target, lone])
    .into();
  let (_tree, layout) = laid_out(root);

  let key = layout.focus_for(panel).expect("a field");
  assert_eq!(
    layout
      .element(&key)
      .and_then(|e| e.label.as_deref()),
    Some("first")
  );
  // Nothing inside can take focus.
  assert!(layout.has_focus_target(empty));
  assert_eq!(layout.focus_for(empty), None);
  assert!(!layout.has_focus_target(FocusTarget::named("missing")));
}
