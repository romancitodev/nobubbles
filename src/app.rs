use std::{io, time::Duration};

use crossterm::event::KeyEvent;
use ratatui::backend::CrosstermBackend;

use crate::components::{Buffer, Rect, Render};

/// A context for the application, holding state and dependencies.
///
/// This struct is passed around to components, so they can access the buffer
/// and other state.
pub struct Ctx<'a> {
  area: Rect,
  buf: &'a mut ratatui::buffer::Buffer,
  key: Option<KeyEvent>,
  wanted_height: u16,
  cursor: Option<(u16, u16)>,
  since: Duration,
}

impl<'a> Ctx<'a> {
  pub(crate) fn new(
    frame: &'a mut ratatui::Frame<'_>,
    key: Option<KeyEvent>,
    since: Duration,
  ) -> Self {
    let area: Rect = frame.area().into();
    Ctx {
      area,
      buf: frame.buffer_mut(),
      key,
      wanted_height: area.height(),
      cursor: None,
      since,
    }
  }

  /// Renders a view into the buffer.
  ///
  /// Takes anything that renders, not just a `Component`, so a bare widget or a `column!`
  /// can be drawn without wrapping it in a struct first. A `Component` passes `c.view()`.
  pub fn render(&mut self, view: impl Render) {
    self.wanted_height = view.height(self.area.width());

    let mut buf = Buffer::from(&mut *self.buf);
    view.render(self.area, &mut buf);
    self.cursor = buf.cursor();
  }

  /// Returns the key event, if one is available.
  pub fn key(&self) -> Option<KeyEvent> {
    self.key
  }

  /// How long since the last frame.
  pub fn since(&self) -> Duration {
    self.since
  }

  /// Runs a [`tachyonfx`] effect over what has been rendered. Needs the `fx` feature.
  ///
  /// Call it after `render`: an effect is a pass over the cells that are already there, so
  /// there has to be something there. It asks for the next frame on its own while it runs.
  ///
  /// ```no_run
  /// # #[cfg(feature = "fx")] {
  /// # use nobubbles::{app::Inline, components::text::Text};
  /// use tachyonfx::{Duration, fx};
  ///
  /// let mut fade = fx::coalesce(Duration::from_millis(600));
  /// Inline::run(30, move |cx| {
  ///   cx.render(Text::new("nobubbles"));
  ///   cx.effect(&mut fade);
  /// })
  /// # ; }
  /// ```
  #[cfg(feature = "fx")]
  pub fn effect(&mut self, effect: &mut tachyonfx::Effect) {
    effect.process(self.since.into(), self.buf, self.area.into());

    if !effect.done() {
      crate::signals::redraw();
    }
  }

  /// Height the last `render()` call reported its view needing, in rows.
  pub(crate) fn wanted_height(&self) -> u16 {
    self.wanted_height
  }

  /// Where the last `render()` asked for the terminal cursor, if anything did.
  pub(crate) fn cursor(&self) -> Option<(u16, u16)> {
    self.cursor
  }
}

/// The user pressed Ctrl+C. Every run returns this instead of dying, so the raw mode guard
/// still gets to drop and the terminal is handed back in one piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("cancelled")
  }
}

impl std::error::Error for Cancelled {}

/// Inline terminal, ideal for CLIs and stuff like that.
pub struct Inline;

/// Starting viewport height, in rows. The real height comes from what the view reports
/// needing (see `Ctx::render`), this is only what the very first frame is measured against.
const INITIAL_HEIGHT: u16 = 1;

impl Inline {
  /// Runs the inline terminal, calling the UI function on each frame. The viewport grows
  /// or shrinks on its own to fit what the view reports needing, there's no height to pass.
  pub fn run(fps: u16, ui: impl FnMut(&mut Ctx)) -> eyre::Result<()> {
    // Before the terminal, not after: building an inline viewport asks the terminal where
    // the cursor is, and on a real tty that answer comes back through stdin. Nested runs
    // share the one guard, so raw mode doesn't flicker between two prompts in a row.
    let _session = crate::inline::Session::open()?;

    let terminal = crate::render::inline(CrosstermBackend::new(io::stdout()), INITIAL_HEIGHT)?;
    let resize = |height: u16| {
      crate::render::inline(CrosstermBackend::new(io::stdout()), height).map_err(Into::into)
    };

    crate::engine::poll(
      terminal,
      INITIAL_HEIGHT,
      resize,
      Duration::from_secs_f32(1.0 / fps.max(1) as f32),
      ui,
    )
  }
}
