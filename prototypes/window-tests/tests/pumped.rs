//! M20 prototype: real-window tests from `cargo test`.
//!
//! This target has `harness = false`, so `main` runs on the process's main
//! thread, as macOS needs. It runs each test in turn on the one event loop,
//! catches a failing test's panic so the rest still run, and prints results
//! as libtest does.
//!
//! It opens windows, so it runs only with `--ignored`:
//! `cargo test -p prototype-window-tests --test pumped -- --ignored`.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use dovetail_render::Color;
use prototype_window_tests::{Driver, second_event_loop};

type Test = fn(&mut Driver) -> Result<String, String>;

const LIMIT: Duration = Duration::from_secs(10);
const BLUE: Color = Color::hex(0x1e66f5);
const GREEN: Color = Color::hex(0x40a02b);

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().collect();
  if !args
    .iter()
    .any(|a| a == "--ignored" || a == "--include-ignored")
  {
    println!("running 0 tests (these open windows: run with -- --ignored)");
    return ExitCode::SUCCESS;
  }
  let main_thread = std::thread::current().name() == Some("main");
  println!("main thread: {main_thread}");

  let start = Instant::now();
  let mut driver = match Driver::new() {
    Ok(d) => d,
    Err(e) => {
      println!("FAIL  {e}");
      return ExitCode::FAILURE;
    },
  };
  let tests: [(&str, Test); 6] = [
    ("a_window_opens_and_draws", opens_and_draws),
    ("a_resized_window_draws_at_its_new_size", resizes),
    ("two_windows_draw_at_once", two_windows),
    ("ten_tests_in_turn_share_the_loop", ten_in_turn),
    ("a_wait_that_never_ends_fails_with_a_message", wait_fails),
    ("a_second_event_loop_is_refused", second_loop),
  ];
  println!("running {} tests", tests.len());
  let mut failed = 0;
  for (name, test) in tests {
    let begun = Instant::now();
    let outcome = catch_unwind(AssertUnwindSafe(|| test(&mut driver)))
      .unwrap_or_else(|_| Err("panicked".to_owned()));
    let took = begun.elapsed();
    match outcome {
      Ok(note) => println!("test {name} ... ok ({took:.0?}) {note}"),
      Err(e) => {
        failed += 1;
        println!("test {name} ... FAILED ({took:.0?}): {e}");
      },
    }
    // Every test leaves no windows behind for the next.
    if driver.open_windows() != 0 {
      failed += 1;
      println!("  {name} left {} windows open", driver.open_windows());
    }
  }
  println!(
    "\ntest result: {}. {} passed; {failed} failed; finished in {:.2?}",
    if failed == 0 { "ok" } else { "FAILED" },
    6 - failed,
    start.elapsed()
  );
  if failed == 0 {
    ExitCode::SUCCESS
  } else {
    ExitCode::FAILURE
  }
}

fn opens_and_draws(d: &mut Driver) -> Result<String, String> {
  let begun = Instant::now();
  let w = d.open("prototype: open", (400.0, 300.0), BLUE)?;
  d.wait_until("a frame", LIMIT, |d| d.seen(w).frames >= 1)?;
  let first_frame = begun.elapsed();
  let seen = d.seen(w);
  d.close(w)?;
  Ok(format!(
    "{}, {}x{} px at scale {}, first frame after {first_frame:.0?}",
    d.adapter().unwrap_or_default(),
    seen.size.width,
    seen.size.height,
    seen.scale
  ))
}

fn resizes(d: &mut Driver) -> Result<String, String> {
  let w = d.open("prototype: resize", (400.0, 300.0), BLUE)?;
  d.wait_until("a frame", LIMIT, |d| d.seen(w).frames >= 1)?;
  let frames = d.seen(w).frames;
  d.resize(w, (640.0, 480.0));
  let scale = d.seen(w).scale;
  let want = (
    (640.0 * scale).round() as u32,
    (480.0 * scale).round() as u32,
  );
  let took = d.wait_until("the new size, drawn", LIMIT, |d| {
    let s = d.seen(w);
    (s.size.width, s.size.height) == want && s.frames > frames
  })?;
  d.close(w)?;
  Ok(format!("{}x{} px after {took:.0?}", want.0, want.1))
}

fn two_windows(d: &mut Driver) -> Result<String, String> {
  let a = d.open("prototype: first", (300.0, 200.0), BLUE)?;
  let b = d.open("prototype: second", (300.0, 200.0), GREEN)?;
  d.wait_until("both windows to draw", LIMIT, |d| {
    d.seen(a).frames >= 1 && d.seen(b).frames >= 1
  })?;
  if d.open_windows() != 2 {
    return Err(format!("{} windows open, not 2", d.open_windows()));
  }
  d.close(b)?;
  if d.open_windows() != 1 {
    return Err("closing one window closed the other".to_owned());
  }
  d.close(a)?;
  Ok(String::new())
}

fn ten_in_turn(d: &mut Driver) -> Result<String, String> {
  let begun = Instant::now();
  for i in 0..10 {
    let w = d.open(&format!("prototype: {i}"), (200.0, 150.0), BLUE)?;
    d.wait_until("a frame", LIMIT, |d| d.seen(w).frames >= 1)?;
    d.close(w)?;
  }
  Ok(format!("{:.0?} for each", begun.elapsed() / 10))
}

fn wait_fails(d: &mut Driver) -> Result<String, String> {
  match d.wait_until(
    "something that never happens",
    Duration::from_millis(200),
    |_| false,
  ) {
    Ok(_) => Err("the wait ended".to_owned()),
    Err(e) if e.contains("timed out") && e.contains("never happens") => {
      Ok(format!("\"{e}\""))
    },
    Err(e) => Err(format!("unexpected message: {e}")),
  }
}

fn second_loop(_: &mut Driver) -> Result<String, String> {
  match second_event_loop() {
    Ok(()) => Err("winit made a second event loop".to_owned()),
    Err(e) => Ok(format!("\"{e}\"")),
  }
}
