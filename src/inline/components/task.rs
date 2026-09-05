use std::sync::mpsc::channel;

use eyre::Result;

use crate::{
  app::Inline,
  components::{
    progress::Progress,
    prompt::{Prompt, PromptState},
    text::Text,
  },
  effects::{self, Emitter},
  signals::{self, quit},
  style::Style,
};

/// A running task's voice. Whatever it says lands on the spinner's line.
pub struct Report(Emitter<String>);

impl Report {
  /// Replaces the line under the title.
  pub fn say(&self, what: impl Into<String>) {
    self.0.send(what.into());
  }
}

/// Runs `work` on a background thread with a spinner on the rail, and collapses to whatever
/// it said last once it's done.
///
/// ```no_run
/// # use nobubbles::inline;
/// let files = inline::task("Building", |report| {
///   report.say("compiling");
///   report.say("linking");
///   42
/// })?;
/// # Ok::<(), eyre::Report>(())
/// ```
///
/// The spinner turns on its own while the task runs, without the task having to tick it:
/// live background work keeps the loop drawing every frame.
///
/// # Errors
/// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C. A task
/// cancelled that way is **not** stopped, it is only stopped being watched: the thread runs
/// to the end on its own.
pub fn task<T: Send + 'static>(
  title: &str,
  work: impl FnOnce(&Report) -> T + Send + 'static,
) -> Result<T> {
  let spinner = Progress::new();
  let said = effects::inbox::<String>();
  let (done, result) = channel();

  said.spawn(move |sender| {
    let value = work(&Report(sender));
    let _ = done.send(value);
  });

  let title = title.to_owned();
  Inline::run(signals::fps(), |cx| {
    let running = said.drain(|line| spinner.set_label(line));

    if running {
      // Nothing new to report is still news while the work is alive.
      spinner.tick();
      cx.render(Prompt::new(PromptState::Active, title.clone(), spinner));
    } else {
      let last = Text::new(spinner.label()).style(Style::new().dim());
      cx.render(Prompt::new(PromptState::Submitted, title.clone(), last));
      quit();
    }
  })?;

  // The sender is dropped when the thread ends, and the loop only stops once it has, so this
  // is already sitting in the channel.
  result
    .recv()
    .map_err(|_| eyre::eyre!("the task ended without a result"))
}

/// Compile-time note: `Report` is what crosses the thread boundary, never a signal. The
/// spinner stays on the loop's thread and only ever sees `String`s (D-018).
const _: fn() = || {
  fn assert_send<T: Send>() {}
  assert_send::<Report>();
};
