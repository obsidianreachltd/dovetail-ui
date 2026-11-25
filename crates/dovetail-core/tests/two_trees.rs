//! Two trees that read the same state, as two windows do (B0005).

use std::cell::Cell;

use dovetail_core::{BuildContext, Component, Signal, Tree, View};

struct Reader;

impl Component for Reader {
  type Props = Signal<i32>;

  fn build(
    value: &Signal<i32>,
    _: &mut BuildContext<'_>,
  ) -> View {
    View::text(format!("{}", value.get()))
  }
}

thread_local! {
  static OWNED: Cell<Option<Signal<i32>>> = const { Cell::new(None) };
}

/// Owns the state, as the main window's app component would.
struct Owner;

impl Component for Owner {
  type Props = ();

  fn build(
    _: &(),
    cx: &mut BuildContext<'_>,
  ) -> View {
    OWNED.set(Some(cx.use_state(|| 1)));
    View::text("owner")
  }
}

fn shown(tree: &Tree) -> Option<View> {
  tree.output(tree.root()?).cloned()
}

#[test]
fn each_tree_keeps_the_other_trees_rebuilds() {
  let mut owner = Tree::new();
  owner.mount(View::component::<Owner>(()));
  let value = OWNED.get().expect("made by the owner");
  let mut first = Tree::new();
  let mut second = Tree::new();
  first.mount(View::component::<Reader>(value));
  second.mount(View::component::<Reader>(value));

  value.set(2);
  // The first tree updates first, and must leave the second tree's mark.
  first.update();
  second.update();
  assert_eq!(shown(&first), Some(View::text("2")));
  assert_eq!(shown(&second), Some(View::text("2")));
}
