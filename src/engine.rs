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

pub fn poll<B: Backend>(
  terminal: &mut ratatui::Terminal<B>,
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
      terminal.draw(|frame| {
        let mut ctx = Ctx::new(frame, key);
        ui(&mut ctx);
      })?;
      crate::signals::clear_dirty();
      last_render = Instant::now();
      first_draw = false;
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
