//! Eight jobs building at once, one row each, plus a spinner on top. When the last one lands
//! the whole thing is replaced by a single line: the viewport shrinks and the bars are gone.
//!
//! Nothing is threaded yet: the loop itself steps every job on each frame. That is enough to
//! see the shape, and the rows keep working unchanged once a worker thread drives them.
//!
//! Ctrl+C aborts and returns `Cancelled`, like in any other run.

use eyre::Result;
use nobubbles::app::Inline;
use nobubbles::components::Column;
use nobubbles::components::progress::Progress;
use nobubbles::components::text::Text;
use nobubbles::signals::quit;
use nobubbles::style::{Color, Style};

const CRATES: [&str; 8] = [
  "serde", "tokio", "ratatui", "eyre", "clap", "regex", "rand", "hyper",
];

fn main() -> Result<()> {
  let header = Progress::new();
  let jobs: Vec<Progress> = CRATES
    .iter()
    .map(|name| Progress::with(format!("Compiling {name}")).width(48))
    .collect();

  Inline::run(30, |cx| {
    for (i, job) in jobs.iter().enumerate() {
      // Each crate builds at its own pace, so the bars don't move in lockstep.
      let step = 0.01 * (i as f32).mul_add(0.4, 1.0);
      job.set(job.ratio().unwrap_or(0.0) + step);
    }

    let done = jobs.iter().filter(|job| job.is_done()).count();

    if done == jobs.len() {
      // Rendering something else is all it takes: the viewport measures one row now, so the
      // loop shrinks it and wipes what the taller one had painted.
      cx.render(Text::new("✨ Compilation done").style(Style::new().fg(Color::LightGreen).bold()));
      quit();
      return;
    }

    header.set_label(format!("Building {done}/{}", jobs.len()));

    cx.render(
      std::iter::once(header)
        .chain(jobs.iter().copied())
        .collect::<Column>(),
    );
  })
}
