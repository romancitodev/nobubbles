use std::{io, time::Duration};

use crossterm::event::KeyEvent;
use ratatui::backend::CrosstermBackend;

use crate::components::{Buffer, Component, Rect, Render};

/// A context for the application, holding state and dependencies.
///
/// This struct is passed around to components, so they can access the buffer
/// and other state.
pub struct Ctx<'a> {
  area: Rect,
  buf: &'a mut ratatui::buffer::Buffer,
  key: Option<KeyEvent>,
}

impl<'a> Ctx<'a> {
  pub(crate) fn new(frame: &'a mut ratatui::Frame<'_>, key: Option<KeyEvent>) -> Self {
    Ctx {
      area: frame.area().into(),
      buf: frame.buffer_mut(),
      key,
    }
  }

  /// Renders the component into the buffer.
  ///
  /// This method is called by the application loop to render the component into the buffer.
  pub fn render(&mut self, component: &impl Component) {
    component
      .view()
      .render(self.area, &mut Buffer::from(&mut *self.buf));
  }

  /// Returns the key event, if one is available.
  pub fn key(&self) -> Option<KeyEvent> {
    self.key
  }
}

/// Inline terminal, ideal for CLIs and stuff like that.
pub struct Inline {
  height: u16,
  fps: u16,
}

impl Inline {
  pub fn new(height: u16, fps: u16) -> Self {
    Inline { height, fps }
  }

  /// Runs the inline terminal, calling the UI function on each frame.
  pub fn run(self, ui: impl FnMut(&mut Ctx)) -> eyre::Result<()> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = crate::render::inline(backend, self.height)?;

    crossterm::terminal::enable_raw_mode()?;
    let result = crate::engine::poll(
      &mut terminal,
      Duration::from_secs_f32(1.0 / self.fps as f32),
      ui,
    );
    crossterm::terminal::disable_raw_mode()?;
    result
  }
}
