//! Measures both prototypes the same way. Run with
//! `cargo run --release -p prototype-reactive-handles`.
//!
//! Each measurement runs 7 times and prints the median.

use std::hint::black_box;
use std::time::{Duration, Instant};

use prototype_reactive_handles::{arena, shared};

const RUNS: usize = 7;
const COMPONENTS: usize = 10_000;
const UPDATES: usize = 1_000_000;
const READS: usize = 1_000_000;

fn median(mut f: impl FnMut() -> Duration) -> Duration {
  let mut times: Vec<Duration> = (0..RUNS).map(|_| f()).collect();
  times.sort();
  times[RUNS / 2]
}

fn per(
  total: Duration,
  count: usize,
) -> String {
  format!("{:>8.1} ns", total.as_nanos() as f64 / count as f64)
}

fn arena_create_dispose() -> Duration {
  let start = Instant::now();
  let root = arena::create_scope(None);
  arena::build(root, || {
    for i in 0..COMPONENTS {
      let child = arena::create_scope(Some(root));
      arena::build(child, || {
        let s = arena::use_state(|| i);
        black_box(s.get());
      });
    }
  });
  arena::dispose(root);
  start.elapsed()
}

fn shared_create_dispose() -> Duration {
  let start = Instant::now();
  let root = shared::create_scope(None);
  shared::build(root, || {
    for i in 0..COMPONENTS {
      let child = shared::create_scope(Some(root));
      shared::build(child, || {
        let s = shared::use_state(|| i);
        black_box(s.get());
      });
    }
  });
  shared::dispose(root);
  start.elapsed()
}

fn arena_updates() -> Duration {
  let root = arena::create_scope(None);
  let s = arena::build(root, || {
    let s = arena::use_state(|| 0u64);
    s.get();
    s
  });
  let start = Instant::now();
  for _ in 0..UPDATES {
    s.update(|v| *v += 1);
  }
  let elapsed = start.elapsed();
  black_box(arena::take_dirty());
  arena::dispose(root);
  elapsed
}

fn shared_updates() -> Duration {
  let root = shared::create_scope(None);
  let s = shared::build(root, || {
    let s = shared::use_state(|| 0u64);
    s.get();
    s
  });
  let start = Instant::now();
  for _ in 0..UPDATES {
    s.update(|v| *v += 1);
  }
  let elapsed = start.elapsed();
  black_box(shared::take_dirty());
  shared::dispose(root);
  elapsed
}

fn arena_reads() -> Duration {
  let root = arena::create_scope(None);
  let elapsed = arena::build(root, || {
    let s = arena::use_state(|| 1u64);
    let start = Instant::now();
    for _ in 0..READS {
      black_box(s.get());
    }
    start.elapsed()
  });
  arena::dispose(root);
  elapsed
}

fn shared_reads() -> Duration {
  let root = shared::create_scope(None);
  let elapsed = shared::build(root, || {
    let s = shared::use_state(|| 1u64);
    let start = Instant::now();
    for _ in 0..READS {
      black_box(s.get());
    }
    start.elapsed()
  });
  shared::dispose(root);
  elapsed
}

fn main() {
  println!(
    "Handle size: arena {} bytes, shared {} bytes",
    std::mem::size_of::<arena::Signal<String>>(),
    std::mem::size_of::<shared::State<String>>()
  );
  println!("Median of {RUNS} runs:");
  println!("| Measurement | Arena (A) | Shared (B) |");
  println!("|---|---|---|");
  println!(
    "| Create, build and remove one component with one state | {} | {} |",
    per(median(arena_create_dispose), COMPONENTS),
    per(median(shared_create_dispose), COMPONENTS)
  );
  println!(
    "| Update a state with one subscriber | {} | {} |",
    per(median(arena_updates), UPDATES),
    per(median(shared_updates), UPDATES)
  );
  println!(
    "| Tracked read while building | {} | {} |",
    per(median(arena_reads), READS),
    per(median(shared_reads), READS)
  );
}
