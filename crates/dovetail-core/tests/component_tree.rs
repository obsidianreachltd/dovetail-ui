//! M02 "Done when" items 2–6, using only the public API in plain Rust.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dovetail_core::{
  BuildContext, Callback, Children, Color, Component, ComponentId, Edges,
  Element, ElementKind, Invalidation, RuntimeStats, Signal, Style, Tree, View,
  runtime_stats,
};

/// Collects values that components hand out, so a test can reach them.
#[derive(Clone)]
struct Inbox<T>(Rc<RefCell<Vec<T>>>);

impl<T> Default for Inbox<T> {
  fn default() -> Self {
    Inbox(Rc::default())
  }
}

impl<T: Clone + 'static> Inbox<T> {
  fn callback(&self) -> Callback<T> {
    let inbox = self.clone();
    Callback::new(move |value| inbox.0.borrow_mut().push(value))
  }

  fn last(&self) -> T {
    self
      .0
      .borrow()
      .last()
      .cloned()
      .expect("a value was sent")
  }
}

// --- 2. Typed properties and children --------------------------------------

#[derive(PartialEq)]
struct PanelProps {
  title: String,
  children: Children,
}

struct Panel;

impl Component for Panel {
  type Props = PanelProps;

  fn build(
    props: &PanelProps,
    _: &mut BuildContext<'_>,
  ) -> View {
    View::fragment(
      std::iter::once(View::text(props.title.clone()))
        .chain(props.children.clone()),
    )
  }
}

struct Label;

impl Component for Label {
  type Props = &'static str;

  fn build(
    text: &&'static str,
    _: &mut BuildContext<'_>,
  ) -> View {
    View::text(*text)
  }
}

#[test]
fn typed_properties_and_children_in_plain_rust() {
  let mut tree = Tree::new();
  let update = tree.mount(View::component::<Panel>(PanelProps {
    title: "Layers".into(),
    children: vec![
      View::component::<Label>("Top"),
      View::component::<Label>("Bottom"),
    ],
  }));
  let root = tree.root().unwrap();
  assert_eq!(update.created.len(), 3);
  assert_eq!(tree.props::<Panel>(root).unwrap().title, "Layers");

  let labels = tree.children(root).to_vec();
  assert_eq!(labels.len(), 2);
  assert_eq!(tree.output(labels[0]), Some(&View::text("Top")));
  assert_eq!(tree.output(labels[1]), Some(&View::text("Bottom")));
  assert_eq!(tree.parent(labels[1]), Some(root));
  assert!(
    tree.props::<Panel>(labels[0]).is_none(),
    "properties are typed"
  );
}

// --- 3. Changing state marks only the affected components ------------------

#[derive(PartialEq)]
struct CounterProps {
  share: Callback<Signal<i32>>,
}

struct Counter;

impl Component for Counter {
  type Props = CounterProps;

  fn build(
    props: &CounterProps,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let count = cx.use_state(|| 0);
    props.share.call(count);
    View::text(count.map(|value| format!("Count: {value}")))
  }
}

struct TwoCounters;

impl Component for TwoCounters {
  type Props = Callback<Signal<i32>>;

  fn build(
    share: &Callback<Signal<i32>>,
    _: &mut BuildContext<'_>,
  ) -> View {
    let counter =
      || View::component::<Counter>(CounterProps { share: share.clone() });
    View::fragment([
      View::text("Two counters"),
      counter(),
      counter(),
    ])
  }
}

#[test]
fn changing_state_marks_only_the_components_that_read_it() {
  let signals = Inbox::default();
  let mut tree = Tree::new();
  tree.mount(View::component::<TwoCounters>(signals.callback()));
  let root = tree.root().unwrap();
  let counters = tree.children(root).to_vec();
  let first = signals.0.borrow()[0];

  first.update(|value| *value += 1);
  let update = tree.update();

  assert_eq!(update.rebuilt, vec![counters[0]]);
  assert_eq!(update.invalidated, vec![(counters[0], Invalidation::ALL)]);
  assert!(update.created.is_empty() && update.removed.is_empty());
  assert_eq!(tree.output(counters[0]), Some(&View::text("Count: 1")));
  assert_eq!(tree.output(counters[1]), Some(&View::text("Count: 0")));
  assert_eq!(tree.update(), Default::default(), "nothing is left marked");
}

#[test]
fn a_change_that_gives_the_same_view_invalidates_nothing() {
  let signals = Inbox::default();
  let mut tree = Tree::new();
  tree.mount(View::component::<TwoCounters>(signals.callback()));
  let first = signals.0.borrow()[0];
  first.set(0);
  let update = tree.update();
  assert_eq!(update.rebuilt.len(), 1);
  assert!(update.invalidated.is_empty());
}

// --- 4. Keyed children keep their state when a list is reordered -----------

#[derive(PartialEq)]
struct ItemProps {
  label: u32,
  share: Callback<(u32, Signal<u32>)>,
}

struct Item;

impl Component for Item {
  type Props = ItemProps;

  fn build(
    props: &ItemProps,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let clicks = cx.use_state(|| 0);
    props.share.call((props.label, clicks));
    View::text(format!("{}:{}", props.label, clicks.get()))
  }
}

#[derive(PartialEq)]
struct ListProps {
  keyed: bool,
  share_items: Callback<Signal<Vec<u32>>>,
  share_clicks: Callback<(u32, Signal<u32>)>,
}

struct List;

impl Component for List {
  type Props = ListProps;

  fn build(
    props: &ListProps,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let items = cx.use_state(|| vec![1, 2, 3]);
    props.share_items.call(items);
    View::fragment(items.get().into_iter().map(|label| {
      let item = View::component::<Item>(ItemProps {
        label,
        share: props.share_clicks.clone(),
      });
      if props.keyed { item.keyed(label) } else { item }
    }))
  }
}

/// The tree, the list's items, and every item's click count by label.
type MountedList = (Tree, Signal<Vec<u32>>, Inbox<(u32, Signal<u32>)>);

fn mount_list(keyed: bool) -> MountedList {
  let items = Inbox::default();
  let clicks = Inbox::default();
  let mut tree = Tree::new();
  tree.mount(View::component::<List>(ListProps {
    keyed,
    share_items: items.callback(),
    share_clicks: clicks.callback(),
  }));
  (tree, items.last(), clicks)
}

fn outputs(
  tree: &Tree,
  parent: ComponentId,
) -> Vec<View> {
  tree
    .children(parent)
    .iter()
    .map(|c| tree.output(*c).unwrap().clone())
    .collect()
}

#[test]
fn keyed_children_keep_their_instance_and_state_when_reordered() {
  let (mut tree, items, clicks) = mount_list(true);
  let root = tree.root().unwrap();
  let before = tree.children(root).to_vec();
  let two = clicks
    .0
    .borrow()
    .iter()
    .find(|(label, _)| *label == 2)
    .unwrap()
    .1;
  two.set(5);
  tree.update();

  items.set(vec![3, 1, 2]);
  let update = tree.update();

  let after = tree.children(root).to_vec();
  assert_eq!(
    after,
    vec![before[2], before[0], before[1]],
    "same instances, new order"
  );
  assert!(update.created.is_empty() && update.removed.is_empty());
  assert_eq!(
    outputs(&tree, root),
    vec![
      View::text("3:0"),
      View::text("1:0"),
      View::text("2:5")
    ]
  );

  items.set(vec![3, 2]);
  let update = tree.update();
  assert_eq!(update.removed, vec![before[0]]);
  assert_eq!(
    outputs(&tree, root),
    vec![View::text("3:0"), View::text("2:5")]
  );
}

#[test]
fn unkeyed_children_are_matched_by_position() {
  let (mut tree, items, clicks) = mount_list(false);
  let root = tree.root().unwrap();
  let two = clicks
    .0
    .borrow()
    .iter()
    .find(|(label, _)| *label == 2)
    .unwrap()
    .1;
  two.set(5);
  items.set(vec![3, 1, 2]);
  tree.update();
  // Without keys the state stays at the second position, not with item 2.
  assert_eq!(
    outputs(&tree, root),
    vec![
      View::text("3:0"),
      View::text("1:5"),
      View::text("2:0")
    ]
  );
}

// --- 5. Removing a component leaves nothing behind -------------------------

struct CountsDrops(Rc<Cell<u32>>);

impl Drop for CountsDrops {
  fn drop(&mut self) {
    self.0.set(self.0.get() + 1);
  }
}

#[derive(PartialEq)]
struct WatcherProps {
  watched: Signal<bool>,
  drops: Rc<Cell<u32>>,
}

struct Watcher;

impl Component for Watcher {
  type Props = WatcherProps;

  fn build(
    props: &WatcherProps,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let drops = props.drops.clone();
    cx.use_state(move || CountsDrops(drops));
    View::text(format!("watching {}", props.watched.get()))
  }
}

struct Toggle;

impl Component for Toggle {
  type Props = (Rc<Cell<u32>>, Callback<Signal<bool>>);

  fn build(
    (drops, share): &Self::Props,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let show = cx.use_state(|| true);
    share.call(show);
    View::when(show.get(), || {
      View::component::<Watcher>(WatcherProps {
        watched: show,
        drops: drops.clone(),
      })
    })
  }
}

#[test]
fn removing_a_component_drops_its_state_and_subscriptions() {
  let drops = Rc::new(Cell::new(0));
  let show = Inbox::default();
  let mut tree = Tree::new();
  tree.mount(View::component::<Toggle>((drops.clone(), show.callback())));
  let root = tree.root().unwrap();
  let watcher = tree.children(root)[0];
  assert_eq!(
    runtime_stats(),
    RuntimeStats {
      components: 2,
      values: 2,
      subscriptions: 2
    }
  );

  show.last().set(false);
  let update = tree.update();

  assert_eq!(update.removed, vec![watcher]);
  assert_eq!(drops.get(), 1, "the removed component's state was dropped");
  assert!(tree.output(watcher).is_none());
  assert_eq!(
    runtime_stats(),
    RuntimeStats {
      components: 1,
      values: 1,
      subscriptions: 1
    }
  );

  let update = tree.unmount();
  assert_eq!(update.removed, vec![root]);
  assert!(tree.is_empty());
  assert_eq!(
    runtime_stats(),
    RuntimeStats::default(),
    "nothing is left behind"
  );
}

// --- 6. A typed event from a child reaches its parent -----------------------

#[derive(PartialEq)]
struct PickerProps {
  on_pick: Callback<u32>,
}

struct Picker;

impl Component for Picker {
  type Props = PickerProps;

  fn build(
    _: &PickerProps,
    _: &mut BuildContext<'_>,
  ) -> View {
    View::text("pick a layer")
  }
}

struct Inspector;

impl Component for Inspector {
  type Props = ();

  fn build(
    _: &(),
    cx: &mut BuildContext<'_>,
  ) -> View {
    let picked = cx.use_state(|| None::<u32>);
    View::fragment([
      View::text(match picked.get() {
        Some(layer) => format!("Picked: {layer}"),
        None => "Nothing picked".to_owned(),
      }),
      View::component::<Picker>(PickerProps {
        on_pick: Callback::new(move |layer| picked.set(Some(layer))),
      }),
    ])
  }
}

#[test]
fn a_typed_event_from_a_child_reaches_its_parent() {
  let mut tree = Tree::new();
  tree.mount(View::component::<Inspector>(()));
  let root = tree.root().unwrap();
  let picker = tree.children(root)[0];

  tree
    .props::<Picker>(picker)
    .unwrap()
    .on_pick
    .call(7);
  let update = tree.update();

  assert!(update.rebuilt.contains(&root));
  assert_eq!(update.invalidated[0], (root, Invalidation::ALL));
  let View::Fragment(views) = tree.output(root).unwrap() else {
    panic!("fragment")
  };
  assert_eq!(views[0], View::text("Picked: 7"));
  assert_eq!(
    tree.children(root),
    [picker],
    "the child keeps its identity"
  );
}

// --- Invalidation says what a change needs ---------------------------------

struct Swatch;

impl Component for Swatch {
  type Props = Callback<Signal<(u32, f32)>>;

  fn build(
    share: &Self::Props,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let state = cx.use_state(|| (0x1e66f5, 10.0));
    share.call(state);
    let (colour, padding) = state.get();
    Element::new(ElementKind::Box)
      .style(Style {
        background: Color::hex(colour),
        padding: Edges::all(padding),
        ..Style::default()
      })
      .children([View::text("swatch")])
      .into()
  }
}

#[test]
fn a_colour_change_needs_paint_only_and_a_size_change_needs_layout() {
  let state = Inbox::default();
  let mut tree = Tree::new();
  tree.mount(View::component::<Swatch>(state.callback()));
  let root = tree.root().unwrap();

  state
    .last()
    .update(|(colour, _)| *colour = 0x40a02b);
  let paint_only = Invalidation { paint: true, ..Invalidation::NONE };
  assert_eq!(tree.update().invalidated, [(root, paint_only)]);

  state
    .last()
    .update(|(_, padding)| *padding = 20.0);
  let layout = Invalidation {
    layout: true,
    paint: true,
    accessibility: false,
  };
  assert_eq!(tree.update().invalidated, [(root, layout)]);
}

// --- Mistakes give clear errors --------------------------------------------

struct Fickle;

impl Component for Fickle {
  type Props = Callback<Signal<bool>>;

  fn build(
    share: &Self::Props,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let flip = cx.use_state(|| false);
    share.call(flip);
    if flip.get() {
      cx.use_state(|| "text");
    } else {
      cx.use_state(|| 0);
    }
    View::Empty
  }
}

#[test]
#[should_panic(expected = "call use_state in the same order every build")]
fn use_state_in_a_different_order_panics_clearly() {
  let flip = Inbox::default();
  let mut tree = Tree::new();
  tree.mount(View::component::<Fickle>(flip.callback()));
  flip.last().set(true);
  tree.update();
}

struct Duplicates;

impl Component for Duplicates {
  type Props = ();

  fn build(
    _: &(),
    _: &mut BuildContext<'_>,
  ) -> View {
    View::fragment([
      View::component::<Label>("a").keyed(1u32),
      View::component::<Label>("b").keyed(1u32),
    ])
  }
}

#[test]
#[should_panic(expected = "have the same key")]
fn duplicate_keys_panic_clearly() {
  Tree::new().mount(View::component::<Duplicates>(()));
}
