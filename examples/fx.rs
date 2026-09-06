//! Text that appears, and text that never stops moving.
//!
//! `cargo run --example fx --features full`
//!
//! Two things that look alike and are not. The ramp is rímel: it recolours the block every
//! time it is composed, so it drifts as long as something redraws. The fade is tachyonfx: a
//! pass over the cells *after* they are painted, which is why `cx.effect` comes after
//! `cx.render` and not instead of it.
//!
//! Neither writes a signal, so neither would ever get a second frame. `redraw()` is what asks
//! for one — an effect asks on its own while it runs, the ramp has to be asked for.

use std::time::Instant;

use eyre::Result;
use nobubbles::app::Inline;
use nobubbles::rimel::{self, Block, Ramp, palette};
use nobubbles::signals::{quit, redraw};
use nobubbles::tachyonfx::{Duration, Effect, Motion, fx};

/// Draws `view` for `seconds`, with `effect` running over it.
fn scene(mut effect: Effect, seconds: f32, view: impl Fn() -> Block) -> Result<()> {
  let started = Instant::now();

  Inline::run(30, move |cx| {
    cx.render(view());
    cx.effect(&mut effect);
    redraw();

    if started.elapsed().as_secs_f32() >= seconds {
      quit();
    }
  })
}

/// A banner that assembles out of noise and then keeps drifting through a pastel ramp.
fn banner() -> Result<()> {
  scene(fx::coalesce(Duration::from_millis(900)), 3.5, || {
    rimel::col([
      rimel::text("nobubbles").animate(Ramp::pastel(), 0.14).bold(),
      rimel::text("signals instead of messages").dim(),
    ])
  })
}

/// A boxed list that sweeps in from the left and dissolves on the way out.
fn boot() -> Result<()> {
  let effect = fx::sequence(&[
    fx::sweep_in(
      Motion::LeftToRight,
      12,
      4,
      palette::BASE,
      Duration::from_millis(900),
    ),
    fx::sleep(Duration::from_millis(1200)),
    fx::dissolve(Duration::from_millis(700)),
  ]);

  scene(effect, 3.2, || {
    rimel::text("engine    ready\nviewport  24 rows\nsignals   3 live\nplugins   none")
      .px(2)
      .py(1)
      .rounded()
      .border_color(palette::MAUVE)
  })
}

fn main() -> Result<()> {
  banner()?;
  println!();
  boot()?;
  Ok(())
}
