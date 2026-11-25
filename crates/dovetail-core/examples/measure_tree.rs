//! Measures the component tree for the M02 report. Run with
//! `cargo run --release -p dovetail-core --example measure_tree`.
//!
//! Each measurement runs 7 times and prints the median.

use std::hint::black_box;
use std::time::{Duration, Instant};

use dovetail_core::{BuildContext, Callback, Component, Signal, Tree, View};

const ITEMS: u32 = 10_000;
const RUNS: usize = 7;

#[derive(PartialEq)]
struct ItemProps {
  label: u32,
}

struct Item;

impl Component for Item {
  type Props = ItemProps;

  fn build(
    props: &ItemProps,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let clicks = cx.use_state(|| 0u32);
    View::text(format!("{}:{}", props.label, clicks.get()))
  }
}

struct List;

impl Component for List {
  type Props = Callback<Signal<Vec<u32>>>;

  fn build(
    share: &Self::Props,
    cx: &mut BuildContext<'_>,
  ) -> View {
    let items = cx.use_state(|| (0..ITEMS).collect::<Vec<_>>());
    share.call(items);
    items.map(|items| {
      View::fragment(items.iter().map(|&label| {
        View::component::<Item>(ItemProps { label }).keyed(label)
      }))
    })
  }
}

fn median(mut f: impl FnMut() -> Duration) -> Duration {
  let mut times: Vec<Duration> = (0..RUNS).map(|_| f()).collect();
  times.sort();
  times[RUNS / 2]
}

fn mount() -> (Tree, Signal<Vec<u32>>) {
  let shared = std::rc::Rc::new(std::cell::Cell::new(None));
  let slot = shared.clone();
  let mut tree = Tree::new();
  tree.mount(View::component::<List>(Callback::new(move |s| {
    slot.set(Some(s))
  })));
  (tree, shared.get().expect("list state"))
}

fn main() {
  let mount_time = median(|| {
    let start = Instant::now();
    let (tree, _) = black_box(mount());
    let elapsed = start.elapsed();
    drop(tree);
    elapsed
  });

  let unmount_time = median(|| {
    let (mut tree, _) = mount();
    let start = Instant::now();
    black_box(tree.unmount());
    start.elapsed()
  });

  let reverse_time = median(|| {
    let (mut tree, items) = mount();
    items.update(|items| items.reverse());
    let start = Instant::now();
    let update = black_box(tree.update());
    let elapsed = start.elapsed();
    assert!(update.created.is_empty() && update.removed.is_empty());
    elapsed
  });

  let no_change_time = median(|| {
    let (mut tree, items) = mount();
    items.update(|_| {});
    let start = Instant::now();
    black_box(tree.update());
    start.elapsed()
  });

  println!("| Measurement ({ITEMS} keyed items, median of {RUNS}) | Time |");
  println!("|---|---|");
  println!(
    "| Mount the list | {:.2} ms |",
    mount_time.as_secs_f64() * 1e3
  );
  println!(
    "| Reverse the list (every instance kept) | {:.2} ms |",
    reverse_time.as_secs_f64() * 1e3
  );
  println!(
    "| Rebuild the list with the same items | {:.2} ms |",
    no_change_time.as_secs_f64() * 1e3
  );
  println!(
    "| Unmount the list | {:.2} ms |",
    unmount_time.as_secs_f64() * 1e3
  );
}
