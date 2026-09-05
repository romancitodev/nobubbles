//! Handing work back to the loop's thread.
//!
//! Signals never cross threads (D-013), so a worker can't touch the view. What crosses is
//! plain data: the worker sends, the loop drains, and the drain runs where the signals live.
//!
//! ```no_run
//! use nobubbles::effects;
//!
//! enum Update { Done(usize) }
//!
//! let inbox = effects::inbox::<Update>();
//! inbox.spawn(|sender| sender.send(Update::Done(3)));
//!
//! // inside the ui closure:
//! let working = inbox.drain(|Update::Done(n)| {
//!   let _ = n; // apply it to your signals here
//! });
//! ```

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

/// The loop's end of an effects channel. Stays on the thread that built it.
pub struct Inbox<T> {
  rx: Receiver<T>,
  /// Kept so `emitter()` has something to clone. It also means the channel never reads as
  /// disconnected, which `try_iter` treats the same as empty anyway.
  tx: Sender<T>,
  alive: Arc<()>,
}

/// Opens a channel for background work to report on.
pub fn inbox<T>() -> Inbox<T> {
  let (tx, rx) = channel();
  Inbox {
    rx,
    tx,
    alive: Arc::new(()),
  }
}

impl<T> Inbox<T> {
  /// Runs `work` on a background thread, handing it a sender.
  ///
  /// The sender is built here, on this thread, so the inbox counts the worker from the call
  /// and not from whenever the thread gets scheduled. Doing it the other way round is a
  /// silent bug: [`Inbox::drain`] would report no work left and the app would wrap up before
  /// the thread had said anything.
  pub fn spawn(&self, work: impl FnOnce(Emitter<T>) + Send + 'static)
  where
    T: Send + 'static,
  {
    let sender = self.emitter();
    std::thread::spawn(move || work(sender));
  }

  /// A sender for one worker. Build it before spawning and move it in.
  ///
  /// It's what [`Inbox::is_live`] counts, so the inbox knows work is coming from the moment
  /// this is called, not from whenever the thread gets scheduled.
  pub fn emitter(&self) -> Emitter<T> {
    Emitter {
      tx: self.tx.clone(),
      _alive: Arc::clone(&self.alive),
    }
  }

  /// Whether any [`Emitter`] is still alive.
  ///
  /// Read it *before* draining. Seeing `false` means every send happened before the last
  /// emitter dropped, so the drain right after catches everything.
  #[must_use]
  pub fn is_live(&self) -> bool {
    Arc::strong_count(&self.alive) > 1
  }

  /// Applies everything the workers sent since the last call, and answers whether more is
  /// coming. Never blocks.
  ///
  /// A `false` is the frame to wrap up on: liveness is read before the messages are applied,
  /// so it can't come back `false` with something still queued behind it. That ordering is
  /// the reason this isn't two calls.
  ///
  /// While work is live it also asks the loop for another frame. Without that the loop would
  /// doze on its idle timeout, and since draining only happens inside the ui closure, a board
  /// that stopped drawing would stop draining too.
  pub fn drain(&self, mut apply: impl FnMut(T)) -> bool {
    let live = self.is_live();
    if live {
      crate::signals::make_dirty();
    }

    for update in self.rx.try_iter() {
      apply(update);
    }
    live
  }
}

/// A worker's end of an [`Inbox`]. `Send`, cloneable, and keeps the loop awake while alive.
pub struct Emitter<T> {
  tx: Sender<T>,
  _alive: Arc<()>,
}

impl<T> Emitter<T> {
  /// Sends an update. A closed channel means the loop already ended, which is not the
  /// worker's problem.
  pub fn send(&self, update: T) {
    let _ = self.tx.send(update);
  }
}

impl<T> Clone for Emitter<T> {
  fn clone(&self) -> Self {
    Self {
      tx: self.tx.clone(),
      _alive: Arc::clone(&self._alive),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_inbox_is_live_only_while_an_emitter_exists() {
    let inbox = inbox::<u8>();
    assert!(!inbox.is_live(), "nobody is working yet");

    let first = inbox.emitter();
    let second = first.clone();
    assert!(inbox.is_live());

    drop(first);
    assert!(inbox.is_live(), "the clone is its own worker");

    drop(second);
    assert!(!inbox.is_live());
  }

  #[test]
  fn drain_takes_what_was_sent_and_stops() {
    let inbox = inbox::<u8>();
    let sender = inbox.emitter();
    let mut seen = Vec::new();

    sender.send(1);
    sender.send(2);
    assert!(inbox.drain(|n| seen.push(n)), "the sender is still alive");
    assert_eq!(seen, vec![1, 2]);

    assert!(
      inbox.drain(|n| seen.push(n)),
      "a second drain finds nothing"
    );
    assert_eq!(seen, vec![1, 2]);
  }

  /// The ordering that makes a `false` safe to act on: the last message is applied in the
  /// same call that reports the work is over.
  #[test]
  fn the_last_message_lands_in_the_frame_that_reports_no_work() {
    let inbox = inbox::<u8>();
    let sender = inbox.emitter();
    let mut seen = Vec::new();

    sender.send(7);
    drop(sender);

    assert!(!inbox.drain(|n| seen.push(n)), "nobody is working any more");
    assert_eq!(seen, vec![7], "and its last word still arrived");
  }

  #[test]
  fn sending_into_a_dropped_inbox_is_not_an_error() {
    let inbox = inbox::<u8>();
    let sender = inbox.emitter();
    drop(inbox);
    sender.send(1); // would panic if it unwrapped
  }
}
