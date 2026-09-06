//! A download, with a bar that walks a colour ramp as it fills.
//!
//! `cargo run --example download --features gradient`
//!
//! The bar is a rímel block, not a [`Progress`](nobubbles::components::progress::Progress):
//! a progress bar paints its fill with one style, and a ramp needs one colour per column.
//! The ramp is laid over the *full* width and then cut to what is done, so the colours stay
//! put instead of stretching as the bar grows.
//!
//! The worker only ever sends a byte count — swap `fetch` for `reader.read(&mut buf)` and the
//! loop below doesn't change. Rate and time left are worked out here, where the clock is.
//!
//! Ctrl+C aborts and returns `Cancelled`.

use std::thread;
use std::time::{Duration, Instant};

use eyre::Result;
use nobubbles::app::Inline;
use nobubbles::effects::{self, Emitter};
use nobubbles::rimel::{self, Block, Ramp, palette};
use nobubbles::signals::quit;

const FILE: &str = "nobubbles-0.1.0.crate";
const TOTAL: u64 = 24_800_000;
/// Columns the bar takes.
const WIDTH: u16 = 48;

/// Where the bytes come from. A real one is a socket; this one is a clock and a bad random
/// number generator.
fn fetch(sender: &Emitter<u64>) {
  let mut left = TOTAL;
  let mut seed = 7u64;

  while left > 0 {
    seed = seed
      .wrapping_mul(6_364_136_223_846_793_005)
      .wrapping_add(1_442_695_040_888_963_407);

    let chunk = (150_000 + (seed >> 33) % 600_000).min(left);
    thread::sleep(Duration::from_millis(40));
    left -= chunk;
    sender.send(chunk);
  }
}

fn bar(ratio: f32) -> Block {
  let filled = (f32::from(WIDTH) * ratio).round().clamp(0.0, f32::from(WIDTH)) as u16;

  rimel::row([
    rimel::text("█".repeat(usize::from(WIDTH)))
      .animate(Ramp::rainbow(), 0.25)
      .w(filled),
    rimel::text("░".repeat(usize::from(WIDTH - filled))).fg(palette::SURFACE1),
  ])
}

fn mb(bytes: u64) -> String {
  format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

fn main() -> Result<()> {
  let inbox = effects::inbox::<u64>();
  inbox.spawn(|sender| fetch(&sender));

  let started = Instant::now();
  let mut have = 0u64;

  Inline::run(30, |cx| {
    // Draining is also what keeps the loop awake: a live worker asks for the next frame, so
    // the ramp keeps drifting without a signal of its own.
    let downloading = inbox.drain(|chunk| have += chunk);
    let elapsed = started.elapsed().as_secs_f32();

    if !downloading {
      let done = format!("✓  {FILE}  —  {} in {elapsed:.1}s", mb(TOTAL));
      cx.render(rimel::text(done).fg(palette::GREEN).bold());
      quit();
      return;
    }

    let ratio = have as f32 / TOTAL as f32;
    let rate = have as f32 / elapsed.max(0.1);
    let left = ((TOTAL - have) as f32 / rate.max(1.0)) as u64;

    cx.render(rimel::col([
      rimel::text(format!("↓  {FILE}"))
        .animate(Ramp::pastel(), 0.12)
        .bold(),
      rimel::row([
        bar(ratio),
        rimel::text(format!("  {:>3.0}%", ratio * 100.0)).bold(),
      ]),
      rimel::text(format!(
        "{} / {}  ·  {}/s  ·  {left}s left",
        mb(have),
        mb(TOTAL),
        mb(rate as u64)
      ))
      .dim(),
    ]));
  })
}
