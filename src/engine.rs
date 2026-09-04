use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyEventKind};
use ratatui::backend::Backend;

use crate::app::Ctx;

const IDLE: Duration = Duration::from_millis(250);

fn poll_timeout(dirty: bool, since: Duration, frame: Duration) -> Duration {
  if dirty {
    frame.saturating_sub(since)
  } else {
    IDLE
  }
}

/// Drives the event loop. `terminal` is owned (not `&mut`) because a resize replaces it
/// wholesale, recreating an inline `Terminal` is the only way to change its viewport
/// height. `resize` builds a fresh one at a given height. `current_height` is what
/// `terminal` was built with, so `poll` can tell when the view asked for something else.
pub fn poll<B: Backend>(
  mut terminal: ratatui::Terminal<B>,
  mut current_height: u16,
  mut resize: impl FnMut(u16) -> eyre::Result<ratatui::Terminal<B>>,
  frame: Duration,
  mut ui: impl FnMut(&mut Ctx),
) -> eyre::Result<()>
where
  B::Error: std::error::Error + Send + Sync + 'static,
{
  let mut last_render = Instant::now();
  let mut first_draw = true;
  loop {
    let timeout = if first_draw {
      Duration::ZERO
    } else {
      poll_timeout(crate::signals::is_dirty(), last_render.elapsed(), frame)
    };
    let key = if crossterm::event::poll(timeout)? {
      let ev = crossterm::event::read()?;
      match ev {
        Event::Key(k) if k.kind == KeyEventKind::Press => Some(k),
        _ => None,
      }
    } else {
      None
    };

    if crate::signals::should_quit() {
      break Ok(());
    }

    if crate::signals::is_dirty() && last_render.elapsed() >= frame || key.is_some() || first_draw {
      let mut wanted_height = current_height;
      terminal.draw(|frame| {
        let mut ctx = Ctx::new(frame, key);
        ui(&mut ctx);
        wanted_height = ctx.wanted_height();
      })?;
      crate::signals::clear_dirty();
      last_render = Instant::now();
      first_draw = false;

      if wanted_height != current_height {
        // Clear the old viewport first, a smaller terminal doesn't know the old one left
        // taller content on screen, so it never overwrites the leftover rows on its own.
        terminal.clear()?;
        // Pin the cursor to column 0 before building the new terminal. If it's left
        // mid-line, the new inline viewport can reserve an extra row to make room for it.
        crossterm::execute!(std::io::stdout(), crossterm::cursor::MoveToColumn(0))?;
        terminal = resize(wanted_height)?;
        current_height = wanted_height;
        first_draw = true; // redraw immediately, at the new size, instead of waiting a frame
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const FRAME: Duration = Duration::from_millis(16);

  #[test]
  fn dirty_before_frame_deadline_waits_the_remainder() {
    let since = Duration::from_millis(10);
    assert_eq!(poll_timeout(true, since, FRAME), Duration::from_millis(6));
  }

  #[test]
  fn dirty_past_frame_deadline_does_not_wait() {
    let since = Duration::from_millis(20);
    assert_eq!(poll_timeout(true, since, FRAME), Duration::ZERO);
  }

  #[test]
  fn not_dirty_always_idles_regardless_of_since() {
    assert_eq!(poll_timeout(false, Duration::ZERO, FRAME), IDLE);
    assert_eq!(poll_timeout(false, Duration::from_secs(5), FRAME), IDLE);
  }
}
