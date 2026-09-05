//! Twenty crates, four cores, one row per core.
//!
//! Each core takes the next crate off a shared queue, walks its bar to the end, and takes
//! another. The rows are signals living on the loop's thread, so the workers never touch
//! them: they send `Update`, which is this example's own protocol, and the loop applies it
//! where the signals live.
//!
//! Ctrl+C aborts and returns `Cancelled`.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eyre::Result;
use nobubbles::app::Inline;
use nobubbles::components::{Column, Render, progress::Progress, text::Text};
use nobubbles::effects::{self, Emitter};
use nobubbles::signals::quit;
use nobubbles::style::{Color, Style};

const CORES: usize = 4;
/// Ticks per crate, so a bar moves instead of jumping.
const STEPS: u32 = 12;
const CRATES: [&str; 20] = [
  "serde",
  "tokio",
  "ratatui",
  "eyre",
  "clap",
  "regex",
  "rand",
  "hyper",
  "bytes",
  "tracing",
  "anyhow",
  "itertools",
  "rayon",
  "reqwest",
  "chrono",
  "uuid",
  "toml",
  "syn",
  "quote",
  "libc",
];

/// The crates still to build. Whoever takes the lock first gets the next one.
type Queue = Arc<Mutex<Vec<&'static str>>>;

/// What a core has to say. The protocol belongs to the app, not to the framework.
enum Update {
  Started { core: usize, name: &'static str },
  Stepped { core: usize, ratio: f32 },
}

impl Update {
  /// Runs on the loop's thread, which is the only place the rows can be written.
  fn apply(self, rows: &[Progress]) {
    match self {
      Self::Started { core, name } => {
        rows[core].set_label(format!("Compiling {name}"));
        rows[core].set(0.0);
      }
      Self::Stepped { core, ratio } => rows[core].set(ratio),
    }
  }
}

/// One core, until the queue runs dry.
fn build(core: usize, queue: &Queue, sender: &Emitter<Update>) {
  loop {
    // `let ... else` and not `while let`, so the guard is dropped at the end of this
    // statement instead of being held across the sleep below.
    let Some(name) = queue.lock().unwrap().pop() else {
      break;
    };

    sender.send(Update::Started { core, name });

    for step in 1..=STEPS {
      thread::sleep(pace(name) / STEPS);
      sender.send(Update::Stepped {
        core,
        ratio: step as f32 / STEPS as f32,
      });
    }
  }
}

/// Fake build time, derived from the name so two runs look the same.
fn pace(name: &str) -> Duration {
  Duration::from_millis(280 + (name.len() as u64 * 50) % 500)
}

/// The rows, with a running count underneath.
fn board(rows: &[Progress], done: usize) -> impl Render {
  let footer =
    Text::new(format!("(Building {done}/{})", CRATES.len())).style(Style::new().dim().italic());

  rows.iter().copied().collect::<Column>().child(footer)
}

fn summary() -> impl Render {
  Text::new(format!(
    "✨ Compiled {} crates on {CORES} cores",
    CRATES.len()
  ))
  .style(Style::new().fg(Color::LightGreen).bold())
}

fn main() -> Result<()> {
  let rows: Vec<Progress> = (0..CORES)
    .map(|_| Progress::with("waiting").width(52))
    .collect();
  let queue: Queue = Arc::new(Mutex::new(CRATES.to_vec()));
  let inbox = effects::inbox::<Update>();

  for core in 0..CORES {
    let queue = Arc::clone(&queue);
    inbox.spawn(move |sender| build(core, &queue, &sender));
  }

  let mut done = 0;

  Inline::run(30, |cx| {
    let building = inbox.drain(|update| {
      // The last tick of a crate is its only ratio of exactly 1.0.
      if matches!(update, Update::Stepped { ratio, .. } if ratio >= 1.0) {
        done += 1;
      }
      update.apply(&rows);
    });

    if building {
      cx.render(board(&rows, done));
    } else {
      cx.render(summary());
      quit();
    }
  })
}
