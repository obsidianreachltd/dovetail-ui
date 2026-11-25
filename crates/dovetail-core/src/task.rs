//! Long-running work on other threads (M11), and timers (M20).
//!
//! [`spawn_task`] runs work on a thread of its own. The work reports
//! progress and checks whether it was cancelled through a [`TaskContext`].
//! Progress and the end are given to callbacks on the UI thread when
//! [`run_task_messages`] is called. The desktop crate calls it when a task
//! wakes it through the waker installed with [`set_task_waker`].
//!
//! [`Task::after`] is a timer on the UI thread, with no thread of its own
//! (D0027). It is due at a time on the UI thread's clock ([`now`]), and
//! [`run_task_messages`] fires the timers that are due. The desktop wakes at
//! [`next_timer`]; a test moves a [`TestClock`](crate::TestClock) instead.
//!
//! [`now`]: crate::now

use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::reactive;
use crate::view::Callback;

/// How far a task has got.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
  /// The part done, from 0 to 1, when the task knows it.
  pub fraction: Option<f32>,
  /// What the task is doing now, for people to read.
  pub message: String,
}

/// How a task ended.
#[derive(Debug, PartialEq)]
pub enum TaskEnd<T> {
  /// The work finished and gave this.
  Done(T),
  /// The work stopped early because it was cancelled.
  Cancelled,
  /// The work panicked, with this message.
  Failed(String),
}

type Wake = Arc<dyn Fn() + Send + Sync>;

static WAKER: Mutex<Option<Wake>> = Mutex::new(None);

/// Installs what wakes the UI thread when a task has a message for it, or
/// removes it with `None`. The desktop crate installs one while it runs.
pub fn set_task_waker(waker: Option<Arc<dyn Fn() + Send + Sync>>) {
  *WAKER
    .lock()
    .unwrap_or_else(PoisonError::into_inner) = waker;
}

fn wake() {
  let waker = WAKER
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .clone();
  if let Some(waker) = waker {
    waker();
  }
}

type Outcome = Result<Option<Box<dyn Any + Send>>, String>;

#[derive(Default)]
struct Inbox {
  progress: Option<Progress>,
  end: Option<Outcome>,
}

#[derive(Default)]
struct Shared {
  cancelled: AtomicBool,
  inbox: Mutex<Inbox>,
}

impl Shared {
  fn inbox(&self) -> std::sync::MutexGuard<'_, Inbox> {
    self
      .inbox
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
  }
}

/// What the work of a task is given: it reports progress, and says
/// whether the task was cancelled. It can be sent to other threads.
#[derive(Clone)]
pub struct TaskContext(Arc<Shared>);

impl TaskContext {
  /// Reports progress: the part done, from 0 to 1, and what the task is
  /// doing. Only the latest report reaches the UI thread, so this can be
  /// called often.
  pub fn progress(
    &self,
    fraction: f32,
    message: impl Into<String>,
  ) {
    self.report(Progress {
      fraction: Some(fraction.clamp(0.0, 1.0)),
      message: message.into(),
    });
  }

  /// Reports progress when the part done is not known.
  pub fn report(
    &self,
    progress: Progress,
  ) {
    let first = self
      .0
      .inbox()
      .progress
      .replace(progress)
      .is_none();
    // The UI thread is woken once for reports that pile up.
    if first {
      wake();
    }
  }

  /// True once the task has been cancelled. Work should check it often,
  /// and return `None` soon after.
  pub fn is_cancelled(&self) -> bool {
    self.0.cancelled.load(Ordering::Relaxed)
  }

  /// Waits for `time`, or less if the task is cancelled meanwhile. True
  /// when the whole time passed.
  pub fn sleep(
    &self,
    time: Duration,
  ) -> bool {
    let step = Duration::from_millis(10);
    let mut left = time;
    while !left.is_zero() {
      if self.is_cancelled() {
        return false;
      }
      let now = left.min(step);
      std::thread::sleep(now);
      left -= now;
    }
    !self.is_cancelled()
  }
}

struct Entry {
  shared: Arc<Shared>,
  on_progress: Option<Callback<Progress>>,
  on_end: Box<dyn Fn(Outcome)>,
  /// For a timer: when it is due on the UI thread's clock.
  due: Option<Instant>,
}

thread_local! {
  static TASKS: RefCell<BTreeMap<u64, Entry>> = const { RefCell::new(BTreeMap::new()) };
  static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}

/// A task started with [`spawn_task`]. It is a small handle that can be
/// copied into closures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Task(u64);

impl Task {
  /// Asks the task to stop. Its work sees this through
  /// [`TaskContext::is_cancelled`].
  pub fn cancel(self) {
    let timer = TASKS.with_borrow(|t| {
      t.get(&self.0).is_some_and(|e| {
        e.shared.cancelled.store(true, Ordering::Relaxed);
        e.due.is_some()
      })
    });
    // A cancelled timer ends at the next run of messages.
    if timer {
      wake();
    }
  }

  /// True until the task's end has been given to its callback.
  pub fn is_running(self) -> bool {
    TASKS.with_borrow(|t| t.contains_key(&self.0))
  }

  /// Calls `then` on the UI thread once `delay` has passed, unless the
  /// returned task is cancelled first.
  ///
  /// The time is on the UI thread's clock ([`now`](crate::now)), and the
  /// callback comes from [`run_task_messages`] once it is due.
  pub fn after(
    delay: Duration,
    then: Callback<()>,
  ) -> Task {
    let id = NEXT.get();
    NEXT.set(id + 1);
    let entry = Entry {
      shared: Arc::new(Shared::default()),
      on_progress: None,
      on_end: Box::new(move |outcome: Outcome| {
        if matches!(outcome, Ok(Some(_))) {
          then.call(());
        }
      }),
      due: Some(crate::clock::now() + delay),
    };
    TASKS.with_borrow_mut(|t| t.insert(id, entry));
    // The new timer is shown as running.
    reactive::notify_change();
    Task(id)
  }
}

/// Runs `work` on a thread of its own. `on_progress` is told what the work
/// reports, and `on_end` how it ended: what it gave, or that it returned
/// `None` (cancelled), or that it panicked. Both are called on this
/// thread, by [`run_task_messages`].
pub fn spawn_task<T: Send + 'static>(
  work: impl FnOnce(&TaskContext) -> Option<T> + Send + 'static,
  on_progress: Option<Callback<Progress>>,
  on_end: Callback<TaskEnd<T>>,
) -> Task {
  let id = NEXT.get();
  NEXT.set(id + 1);
  let shared = Arc::new(Shared::default());
  let entry = Entry {
    shared: Arc::clone(&shared),
    on_progress,
    on_end: Box::new(move |outcome: Outcome| {
      let end = match outcome {
        Ok(Some(value)) => match value.downcast::<T>() {
          Ok(value) => TaskEnd::Done(*value),
          Err(_) => TaskEnd::Failed("the result had the wrong type".into()),
        },
        Ok(None) => TaskEnd::Cancelled,
        Err(message) => TaskEnd::Failed(message),
      };
      on_end.call(end);
    }),
    due: None,
  };
  TASKS.with_borrow_mut(|t| t.insert(id, entry));
  let cx = TaskContext(shared);
  let spawned = std::thread::Builder::new()
    .name(format!("dovetail-task-{id}"))
    .spawn(move || {
      let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&cx)));
      let outcome = match result {
        Ok(value) => Ok(value.map(|v| Box::new(v) as Box<dyn Any + Send>)),
        Err(panic) => Err(panic_message(&*panic)),
      };
      cx.0.inbox().end = Some(outcome);
      wake();
    });
  if let Err(e) = spawned {
    let shared = TASKS.with_borrow(|t| Arc::clone(&t[&id].shared));
    shared.inbox().end = Some(Err(format!("the task could not start: {e}")));
  }
  // The new task is shown as running.
  reactive::notify_change();
  Task(id)
}

fn panic_message(panic: &(dyn Any + Send)) -> String {
  if let Some(s) = panic.downcast_ref::<&str>() {
    (*s).to_owned()
  } else if let Some(s) = panic.downcast_ref::<String>() {
    s.clone()
  } else {
    "the task panicked".into()
  }
}

/// Gives the tasks' progress and ends to their callbacks. Returns how many
/// messages there were. The desktop crate calls it when a task wakes it;
/// tests without a desktop call it themselves.
pub fn run_task_messages() -> usize {
  let mut count = run_timers_due(crate::clock::now());
  let ids: Vec<u64> = TASKS.with_borrow(|t| {
    t.iter()
      .filter(|(_, e)| e.due.is_none())
      .map(|(id, _)| *id)
      .collect()
  });
  for id in ids {
    let Some((shared, on_progress)) = TASKS.with_borrow(|t| {
      t.get(&id)
        .map(|e| (Arc::clone(&e.shared), e.on_progress.clone()))
    }) else {
      continue;
    };
    let (progress, end) = {
      let mut inbox = shared.inbox();
      (inbox.progress.take(), inbox.end.take())
    };
    if let (Some(progress), Some(on_progress)) = (progress, on_progress) {
      count += 1;
      on_progress.call(progress);
    }
    if let Some(end) = end {
      count += 1;
      // Removed first, so the callback sees the task as ended.
      if let Some(entry) = TASKS.with_borrow_mut(|t| t.remove(&id)) {
        (entry.on_end)(end);
      }
    }
  }
  count
}

/// How many tasks have not ended yet, timers included.
pub fn tasks_running() -> usize {
  TASKS.with_borrow(BTreeMap::len)
}

/// How many timers from [`Task::after`] are waiting.
pub fn timers_waiting() -> usize {
  TASKS.with_borrow(|t| t.values().filter(|e| e.due.is_some()).count())
}

/// When the next timer is due, on the UI thread's clock. A cancelled timer
/// is due at once, so it ends.
pub fn next_timer() -> Option<Instant> {
  let now = crate::clock::now();
  TASKS.with_borrow(|t| {
    t.values()
      .filter_map(|e| {
        let due = e.due?;
        Some(if e.shared.cancelled.load(Ordering::Relaxed) {
          now
        } else {
          due
        })
      })
      .min()
  })
}

/// When the first timer not cancelled that is due by `limit` is due.
pub(crate) fn next_due_by(limit: Instant) -> Option<Instant> {
  TASKS.with_borrow(|t| {
    t.values()
      .filter(|e| !e.shared.cancelled.load(Ordering::Relaxed))
      .filter_map(|e| e.due)
      .filter(|&due| due <= limit)
      .min()
  })
}

/// Fires the timers due by `now`, earliest first, and ends the cancelled
/// ones. Returns how many ended.
pub(crate) fn run_timers_due(now: Instant) -> usize {
  let mut due: Vec<(Instant, u64)> = TASKS.with_borrow(|t| {
    t.iter()
      .filter_map(|(id, e)| {
        let at = e.due?;
        let cancelled = e.shared.cancelled.load(Ordering::Relaxed);
        (cancelled || at <= now).then_some((at, *id))
      })
      .collect()
  });
  due.sort();
  let mut count = 0;
  for (_, id) in due {
    // Removed first, so the callback sees the timer as ended.
    let Some(entry) = TASKS.with_borrow_mut(|t| t.remove(&id)) else {
      continue;
    };
    count += 1;
    let fired = !entry.shared.cancelled.load(Ordering::Relaxed);
    let outcome: Outcome = if fired {
      Ok(Some(Box::new(()) as Box<dyn Any + Send>))
    } else {
      Ok(None)
    };
    (entry.on_end)(outcome);
  }
  count
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::rc::Rc;
  use std::time::Instant;

  fn wait_for(task: Task) {
    let start = Instant::now();
    while task.is_running() {
      assert!(start.elapsed() < Duration::from_secs(10), "task too slow");
      run_task_messages();
      std::thread::sleep(Duration::from_millis(5));
    }
  }

  #[test]
  fn a_task_reports_progress_and_its_result() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let ended = Rc::new(RefCell::new(None));
    let (s, e) = (Rc::clone(&seen), Rc::clone(&ended));
    let task = spawn_task(
      |cx| {
        for i in 1..=4 {
          cx.progress(i as f32 / 4.0, format!("step {i}"));
          std::thread::sleep(Duration::from_millis(20));
        }
        Some(42)
      },
      Some(Callback::new(move |p: Progress| s.borrow_mut().push(p))),
      Callback::new(move |end| *e.borrow_mut() = Some(end)),
    );
    assert!(task.is_running());
    wait_for(task);
    assert_eq!(*ended.borrow(), Some(TaskEnd::Done(42)));
    let seen = seen.borrow();
    assert!(!seen.is_empty(), "some progress was seen");
    assert!(
      seen
        .windows(2)
        .all(|w| w[0].fraction <= w[1].fraction),
      "progress only goes up: {seen:?}"
    );
  }

  #[test]
  fn a_cancelled_task_ends_as_cancelled() {
    let ended = Rc::new(RefCell::new(None));
    let e = Rc::clone(&ended);
    let task = spawn_task(
      |cx| {
        while !cx.is_cancelled() {
          std::thread::sleep(Duration::from_millis(2));
        }
        None::<()>
      },
      None,
      Callback::new(move |end| *e.borrow_mut() = Some(end)),
    );
    std::thread::sleep(Duration::from_millis(30));
    run_task_messages();
    assert!(task.is_running(), "still running before the cancel");
    task.cancel();
    wait_for(task);
    assert_eq!(*ended.borrow(), Some(TaskEnd::Cancelled));
  }

  #[test]
  fn a_panic_ends_the_task_as_failed() {
    let ended = Rc::new(RefCell::new(None));
    let e = Rc::clone(&ended);
    let task = spawn_task(
      |_| -> Option<()> { panic!("broken on purpose") },
      None,
      Callback::new(move |end| *e.borrow_mut() = Some(end)),
    );
    wait_for(task);
    assert_eq!(
      *ended.borrow(),
      Some(TaskEnd::Failed("broken on purpose".into()))
    );
  }

  #[test]
  fn after_calls_back_unless_cancelled() {
    let calls = Rc::new(std::cell::Cell::new(0));
    let c = Rc::clone(&calls);
    let kept = Task::after(
      Duration::from_millis(20),
      Callback::new(move |()| c.set(c.get() + 1)),
    );
    let c = Rc::clone(&calls);
    let dropped = Task::after(
      Duration::from_secs(5),
      Callback::new(move |()| c.set(c.get() + 100)),
    );
    dropped.cancel();
    wait_for(kept);
    wait_for(dropped);
    assert_eq!(calls.get(), 1);
  }

  #[test]
  fn a_test_clock_fires_timers_in_order_at_their_time() {
    let clock = crate::TestClock::install();
    let start = crate::now();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let timer = |ms: u64, name: &'static str| {
      let s = Rc::clone(&seen);
      Task::after(
        Duration::from_millis(ms),
        Callback::new(move |()| {
          s.borrow_mut().push((name, crate::now() - start));
        }),
      )
    };
    timer(300, "late");
    let first = timer(100, "early");
    let cancelled = timer(200, "cancelled");
    cancelled.cancel();
    assert_eq!(
      next_timer(),
      Some(start),
      "the cancelled timer ends at once"
    );
    // Nothing fires while the clock stands still, however long it takes.
    std::thread::sleep(Duration::from_millis(20));
    run_task_messages();
    assert!(seen.borrow().is_empty());
    assert!(!cancelled.is_running());
    assert_eq!(next_timer(), Some(start + Duration::from_millis(100)));

    assert_eq!(clock.advance(Duration::from_millis(250)), 1);
    assert!(!first.is_running());
    assert_eq!(clock.advance(Duration::from_millis(250)), 1);
    assert_eq!(
      *seen.borrow(),
      [
        ("early", Duration::from_millis(100)),
        ("late", Duration::from_millis(300)),
      ]
    );
    assert_eq!(crate::now() - start, Duration::from_millis(500));
    assert_eq!(next_timer(), None);
    crate::set_clock(None);
  }

  #[test]
  fn a_timer_that_sets_another_fires_both_in_one_advance() {
    let clock = crate::TestClock::install();
    let count = Rc::new(std::cell::Cell::new(0));
    fn again(count: Rc<std::cell::Cell<u32>>) {
      let c = Rc::clone(&count);
      Task::after(
        Duration::from_millis(40),
        Callback::new(move |()| {
          c.set(c.get() + 1);
          if c.get() < 5 {
            again(Rc::clone(&c));
          }
        }),
      );
    }
    again(Rc::clone(&count));
    // Five frames 40 ms apart, as an animation sets its next frame.
    assert_eq!(clock.advance(Duration::from_secs(1)), 5);
    assert_eq!(count.get(), 5);
    crate::set_clock(None);
  }
}
