use std::io::{self, Stdout};

use crossterm::terminal::{
  EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::{
  Terminal, TerminalOptions, Viewport,
  backend::{Backend, CrosstermBackend},
};

/// It creates an `inline` Terminal backed up from `ratatui`.
pub(crate) fn inline<B: Backend>(backend: B, height: u16) -> Result<Terminal<B>, B::Error> {
  Terminal::with_options(
    backend,
    TerminalOptions {
      viewport: Viewport::Inline(height),
    },
  )
}

#[allow(dead_code, reason = "wired up by the fullscreen entry point, Fase 6")]
pub(crate) fn fullscreen<B: Backend>(backend: B) -> Result<Terminal<B>, B::Error> {
  Terminal::new(backend)
}

/// Enables raw mode, enters the alt screen, and builds a fullscreen Terminal.
/// TODO: no panic guard yet, that belongs with the Fase 3 loop.
#[allow(dead_code, reason = "wired up by the fullscreen entry point, Fase 6")]
pub(crate) fn enter_fullscreen() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
  enable_raw_mode()?;
  crossterm::execute!(io::stdout(), EnterAlternateScreen)?;
  fullscreen(CrosstermBackend::new(io::stdout()))
}

/// Reverse order of `enter_fullscreen`.
#[allow(dead_code, reason = "wired up by the fullscreen entry point, Fase 6")]
pub(crate) fn leave_fullscreen() -> io::Result<()> {
  crossterm::execute!(io::stdout(), LeaveAlternateScreen)?;
  disable_raw_mode()
}

#[cfg(test)]
mod tests {
  use ratatui::backend::TestBackend;

  use super::*;

  #[test]
  fn inline_viewport_gets_the_requested_height() {
    let backend = TestBackend::new(20, 10);

    let mut terminal = inline(backend, 3).unwrap();
    let mut height = 0;
    let _ = terminal
      .draw(|f| {
        height = f.area().height;
      })
      .unwrap();
    assert_eq!(height, 3);
  }

  #[test]
  fn fullscreen_viewport_covers_the_whole_backend() {
    let backend = TestBackend::new(20, 10);

    let mut terminal = fullscreen(backend).unwrap();
    let mut area = ratatui::layout::Rect::default();
    let _ = terminal
      .draw(|f| {
        area = f.area();
      })
      .unwrap();

    assert_eq!(area, ratatui::layout::Rect::new(0, 0, 20, 10));
  }

  #[test]
  fn inline_grows_and_shrinks_by_recreating_the_terminal() {
    for height in [1, 2, 3, 4, 5, 4, 3, 2, 1] {
      let mut terminal = inline(TestBackend::new(20, 10), height).unwrap();
      let mut got = 0;
      let _ = terminal.draw(|f| got = f.area().height).unwrap();
      assert_eq!(got, height);
    }
  }

  #[test]
  fn insert_before_pushes_the_viewport_down() {
    use ratatui::layout::Position;
    use ratatui::style::Style;

    let mut backend = TestBackend::with_lines([
      "0000000000",
      "1111111111",
      "2222222222",
      "3333333333",
      "4444444444",
      "5555555555",
      "6666666666",
      "7777777777",
      "8888888888",
      "9999999999",
    ]);
    backend
      .set_cursor_position(Position { x: 0, y: 3 })
      .unwrap();

    let mut terminal = inline(backend, 4).unwrap();

    terminal
      .insert_before(1, |buf| {
        buf.set_string(0, 0, "INSERTLINE", Style::default());
      })
      .unwrap();

    terminal.backend().assert_buffer_lines([
      "0000000000",
      "1111111111",
      "2222222222",
      "INSERTLINE",
      "          ",
      "          ",
      "          ",
      "          ",
      "          ",
      "          ",
    ]);
  }
}
