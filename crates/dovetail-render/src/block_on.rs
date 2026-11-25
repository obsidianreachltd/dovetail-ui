//! A minimal way to wait for a future on the current thread.
//!
//! wgpu asks for the adapter and device with futures. On desktop platforms
//! these finish almost at once, so a tiny executor is enough and saves an
//! extra dependency (D0002).

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
  fn wake(self: Arc<Self>) {
    self.0.unpark();
  }

  fn wake_by_ref(self: &Arc<Self>) {
    self.0.unpark();
  }
}

/// Runs `future` to completion on the current thread.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
  let mut future = pin!(future);
  let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
  let mut cx = Context::from_waker(&waker);
  loop {
    match future.as_mut().poll(&mut cx) {
      Poll::Ready(output) => return output,
      Poll::Pending => thread::park(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::block_on;

  #[test]
  fn ready_future_returns_its_value() {
    assert_eq!(block_on(async { 7 }), 7);
  }

  #[test]
  fn future_woken_from_another_thread_finishes() {
    use std::sync::mpsc;
    use std::task::Poll;

    let (tx, rx) = mpsc::channel::<()>();
    let mut started = false;
    let value = block_on(std::future::poll_fn(move |cx| {
      if rx.try_recv().is_ok() {
        return Poll::Ready(42);
      }
      if !started {
        started = true;
        let waker = cx.waker().clone();
        let tx = tx.clone();
        std::thread::spawn(move || {
          tx.send(()).unwrap();
          waker.wake();
        });
      }
      Poll::Pending
    }));
    assert_eq!(value, 42);
  }
}
