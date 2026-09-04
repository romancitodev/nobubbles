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
  wanted_height: u16,
}

impl<'a> Ctx<'a> {
  pub(crate) fn new(frame: &'a mut ratatui::Frame<'_>, key: Option<KeyEvent>) -> Self {
    let area: Rect = frame.area().into();
    Ctx {
      area,
      buf: frame.buffer_mut(),
      key,
      wanted_height: area.height(),
    }
  }

  /// Renders the component into the buffer.
  ///
  /// This method is called by the application loop to render the component into the buffer.
  pub fn render(&mut self, component: &impl Component) {
    let view = component.view();
    self.wanted_height = view.height(self.area.width());
    view.render(self.area, &mut Buffer::from(&mut *self.buf));
  }

  /// Returns the key event, if one is available.
  pub fn key(&self) -> Option<KeyEvent> {
    self.key
  }

  /// Height the last `render()` call reported its component needing, in rows.
  pub(crate) fn wanted_height(&self) -> u16 {
    self.wanted_height
  }
}

/// Inline terminal, ideal for CLIs and stuff like that.
pub struct Inline;

/// Starting viewport height, in rows. The real height comes from what the view reports
/// needing (see `Ctx::render`), this is only what the very first frame is measured against.
const INITIAL_HEIGHT: u16 = 1;

impl Inline {
  /// Runs the inline terminal, calling the UI function on each frame. The viewport grows
  /// or shrinks on its own to fit what the view reports needing, there's no height to pass.
  pub fn run(fps: u16, ui: impl FnMut(&mut Ctx)) -> eyre::Result<()> {
    let terminal = crate::render::inline(CrosstermBackend::new(io::stdout()), INITIAL_HEIGHT)?;
    let resize = |height: u16| {
      crate::render::inline(CrosstermBackend::new(io::stdout()), height).map_err(Into::into)
    };

    crossterm::terminal::enable_raw_mode()?;
    let result = crate::engine::poll(
      terminal,
      INITIAL_HEIGHT,
      resize,
      Duration::from_secs_f32(1.0 / fps as f32),
      ui,
    );
    crossterm::terminal::disable_raw_mode()?;
    result
  }
}
