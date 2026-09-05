use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::Backend, layout::Position};

use crate::app::{Cancelled, Ctx};

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
  crate::signals::clear_quit();

  let mut last_render = Instant::now();
  let mut first_draw = true;
  let mut origin = 0;
  let mut cancelled = false;

  loop {
    // Checked before the poll and not after, so a `quit()` from the last frame doesn't sit
    // through a whole idle timeout on the way out — with prompts running back to back that
    // lag is the part the user feels. The `first_draw` half holds the exit off until a frame
    // a resize just scheduled has actually been painted: without it, a view that shrinks and
    // quits on the same keystroke leaves an empty viewport behind.
    if !first_draw && crate::signals::should_quit() {
      break;
    }

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

    if key.is_some_and(|k| is_cancel(&k)) {
      cancelled = true;
      break;
    }

    if crate::signals::is_dirty() && last_render.elapsed() >= frame || key.is_some() || first_draw {
      let mut wanted_height = current_height;
      // Cleared before `ui`, never after. What the closure writes while drawing is news for
      // the *next* frame, and clearing afterwards threw it away: a view that animates itself
      // went dirty, got wiped, and then sat idle forever because only `ui` can dirty it and
      // only a dirty flag runs `ui`.
      crate::signals::clear_dirty();
      terminal.draw(|frame| {
        origin = frame.area().y;
        let mut ctx = Ctx::new(frame, key);
        ui(&mut ctx);
        wanted_height = ctx.wanted_height();
      })?;
      last_render = Instant::now();
      first_draw = false;

      if wanted_height != current_height {
        // Clear the old viewport first, a smaller terminal doesn't know the old one left
        // taller content on screen, so it never overwrites the leftover rows on its own.
        reanchor(&mut terminal, origin)?;
        terminal = resize(wanted_height)?;
        current_height = wanted_height;
        first_draw = true; // redraw immediately, at the new size, instead of waiting a frame
      }
    }
  }

  park_below(&mut terminal, origin, current_height)?;

  if cancelled {
    return Err(Cancelled.into());
  }
  Ok(())
}

/// Ctrl+C. Raw mode turns it into a plain key event instead of an interrupt, so a view that
/// binds no quit key would have no way out, and a prompt would just type a `c`.
fn is_cancel(key: &KeyEvent) -> bool {
  key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Leaves the cursor on the row right below the viewport, so whatever runs next — the next
/// inline prompt, or the shell once the process ends — starts under what this run painted
/// instead of on top of it. That handoff is the whole prompt transcript: each run leaves its
/// last frame on screen and steps past it.
///
/// `append_lines` is what makes it safe at the bottom of the screen, where there's no row
/// below to move to and the terminal has to scroll instead.
///
/// ponytail: inline-shaped, and `poll` only drives inline today. Fullscreen (Fase 6) leaves
/// through the alt screen instead and will want this skipped.
fn park_below<B: Backend>(
  terminal: &mut Terminal<B>,
  origin: u16,
  height: u16,
) -> Result<(), B::Error> {
  terminal.set_cursor_position(Position::new(0, origin + height.saturating_sub(1)))?;
  terminal.backend_mut().append_lines(1)?;
  terminal.backend_mut().flush()
}

fn reanchor<B: Backend>(terminal: &mut Terminal<B>, origin: u16) -> Result<(), B::Error> {
  terminal.clear()?;
  terminal.set_cursor_position(Position::new(0, origin))?;
  terminal.backend_mut().flush()?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use ratatui::{
    backend::TestBackend,
    widgets::{Paragraph, Widget},
  };

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

  /// `TestBackend` clones its screen and its cursor, which is the state a real backend would
  /// carry over on its own — `Terminal` gives no way to hand the original one to the next
  /// terminal, so this is how the two halves of a resize get chained in a test.
  fn next_terminal(terminal: Terminal<TestBackend>, height: u16) -> Terminal<TestBackend> {
    let backend = terminal.backend().clone();
    drop(terminal);
    crate::render::inline(backend, height).unwrap()
  }

  fn draw(terminal: &mut Terminal<TestBackend>, text: &'static str) {
    terminal
      .draw(|frame| Paragraph::new(text).render(frame.area(), frame.buffer_mut()))
      .unwrap();
  }

  /// Shrinking is a clear + rebuild, and the rebuilt `Terminal` anchors itself wherever the
  /// cursor is. Row 1 in, row 1 out — without `reanchor` the smaller view lands on row 2,
  /// because `Terminal::clear` puts the cursor back where the last draw left it.
  #[test]
  fn shrinking_keeps_the_viewport_on_its_original_row() {
    let mut backend = TestBackend::new(10, 4);
    backend.set_cursor_position(Position::new(0, 1)).unwrap();

    let mut terminal = crate::render::inline(backend, 2).unwrap();
    draw(&mut terminal, "top\nbottom");

    reanchor(&mut terminal, 1).unwrap();
    let mut terminal = next_terminal(terminal, 1);
    draw(&mut terminal, "done");

    terminal.backend().assert_buffer_lines([
      "          ",
      "done      ",
      "          ",
      "          ",
    ]);
  }

  /// The prompt transcript: one run leaves its frame painted and steps past it, so the next
  /// one anchors underneath instead of on top.
  #[test]
  fn park_below_leaves_the_next_viewport_under_this_one() {
    let mut terminal = crate::render::inline(TestBackend::new(10, 4), 2).unwrap();
    draw(&mut terminal, "top\nbottom");

    park_below(&mut terminal, 0, 2).unwrap();
    let mut terminal = next_terminal(terminal, 1);
    draw(&mut terminal, "next");

    terminal.backend().assert_buffer_lines([
      "top       ",
      "bottom    ",
      "next      ",
      "          ",
    ]);
  }
}
