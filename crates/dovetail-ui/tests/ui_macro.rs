//! M03 "Done when" items 1, 2, 3 and 5, through the facade as an application
//! uses it.

use dovetail::prelude::*;
// A component from another crate, after an ordinary `use` (item 3).
use dovetail_controls::Button;
use dovetail_controls::{Column, ColumnProps, Text, TextProps};

/// The example from docs/architecture/syntax.md, as agreed in M03 (item 1).
#[component]
fn Counter() -> View {
  let count = use_state(|| 0);

  ui! {
    Column {
      Text {
        value: count.map(|value| format!("Count: {value}")),
      }

      Button {
        text: "Increment",
        on_click: move |_| count.update(|value| *value += 1),
      }
    }
  }
}

fn only_child(
  tree: &Tree,
  id: ComponentId,
) -> ComponentId {
  match tree.children(id) {
    [child] => *child,
    other => panic!("expected one child, found {}", other.len()),
  }
}

#[test]
fn counter_example_compiles_and_counts() {
  let mut tree = Tree::new();
  tree.mount(ui! { Counter {} });
  let counter = tree.root().unwrap();
  let column = only_child(&tree, counter);
  let [text, button] = tree.children(column) else {
    panic!("text and button")
  };
  let (text, button) = (*text, *button);
  assert_eq!(tree.output(text), Some(&View::text("Count: 0")));
  assert_eq!(tree.props::<Button>(button).unwrap().text, "Increment");

  tree
    .props::<Button>(button)
    .unwrap()
    .on_click
    .call(());
  tree
    .props::<Button>(button)
    .unwrap()
    .on_click
    .call(());
  let update = tree.update();

  assert_eq!(tree.output(text), Some(&View::text("Count: 2")));
  assert_eq!(tree.children(column), [text, button], "identities are kept");
  assert!(update.rebuilt.contains(&counter));
}

/// The macro's expansion builds exactly what hand-written M02 code builds
/// (item 2).
#[test]
fn expansion_equals_hand_written_code() {
  let from_macro = ui! {
    Column {
      Text { value: "a" }
      "b"
      { String::from("c") }
    }
  };
  let by_hand = View::component::<Column>(ColumnProps {
    children: vec![
      View::component::<Text>(TextProps {
        value: "a".to_owned(),
        size: None,
        color: None,
      }),
      View::text("b"),
      View::text("c"),
    ],
    gap: 8.0,
  });
  assert_eq!(from_macro, by_hand);
}

#[component]
fn Badge(
  #[prop(into)] label: String,
  #[prop(default)] count: u32,
  #[prop(into, default = "grey")] colour: String,
) -> View {
  ui! { { format!("{label} {count} {colour}") } }
}

#[test]
fn optional_properties_take_their_defaults() {
  let mut tree = Tree::new();
  tree.mount(ui! { Column { Badge { label: "new" } Badge { label: "hot", count: 3, colour: "red" } } });
  let column = tree.root().unwrap();
  let outputs: Vec<_> = tree
    .children(column)
    .iter()
    .map(|c| tree.output(*c).unwrap().clone())
    .collect();
  assert_eq!(
    outputs,
    [
      View::text("new 0 grey"),
      View::text("hot 3 red")
    ]
  );
}

/// Conditional content and keyed lists (item 5).
#[component]
fn Layers(
  show_hidden: bool,
  layers: Vec<(u32, String, bool)>,
) -> View {
  ui! {
    if layers.is_empty() {
      "No layers"
    } else if show_hidden {
      for (id, name, _) in layers.clone(), key = id {
        Text { value: name }
      }
    } else {
      for (id, name, _) in layers.iter().filter(|layer| !layer.2).cloned(), key = id {
        Text { value: name }
      }
    }
  }
}

fn texts(
  tree: &Tree,
  id: ComponentId,
) -> Vec<(ComponentId, View)> {
  tree
    .children(id)
    .iter()
    .map(|c| (*c, tree.output(*c).unwrap().clone()))
    .collect()
}

#[test]
fn conditional_content_and_keyed_lists() {
  let layers = vec![
    (1, "Top".to_owned(), false),
    (2, "Inner".to_owned(), true),
    (3, "Bottom".to_owned(), false),
  ];
  let mut tree = Tree::new();

  tree.mount(ui! { Layers { show_hidden: true, layers: layers.clone() } });
  let root = tree.root().unwrap();
  let all = texts(&tree, root);
  assert_eq!(
    all
      .iter()
      .map(|(_, v)| v.clone())
      .collect::<Vec<_>>(),
    [
      View::text("Top"),
      View::text("Inner"),
      View::text("Bottom")
    ]
  );

  tree.mount(ui! { Layers { show_hidden: false, layers: layers.clone() } });
  let root = tree.root().unwrap();
  let visible = texts(&tree, root);
  assert_eq!(
    visible
      .iter()
      .map(|(_, v)| v.clone())
      .collect::<Vec<_>>(),
    [View::text("Top"), View::text("Bottom")]
  );

  tree.mount(ui! { Layers { show_hidden: false, layers: vec![] } });
  assert_eq!(
    tree.output(tree.root().unwrap()),
    Some(&View::text("No layers"))
  );
}

/// A keyed `for` keeps each item's instance when the list is reordered.
#[component]
fn Reorderable() -> View {
  let order = use_state(|| vec![1u32, 2, 3]);
  let reverse = move |_: ()| order.update(|o| o.reverse());
  ui! {
    Column {
      Button { text: "Reverse", on_click: reverse }
      for id in order.get(), key = id {
        Text { value: id.to_string() }
      }
    }
  }
}

#[test]
fn keyed_for_keeps_instances_when_reordered() {
  let mut tree = Tree::new();
  tree.mount(ui! { Reorderable {} });
  let column = only_child(&tree, tree.root().unwrap());
  let before = tree.children(column).to_vec();
  let button = before[0];

  tree
    .props::<Button>(button)
    .unwrap()
    .on_click
    .call(());
  let update = tree.update();

  assert_eq!(
    tree.children(column),
    [button, before[3], before[2], before[1]]
  );
  assert!(update.created.is_empty() && update.removed.is_empty());
}
