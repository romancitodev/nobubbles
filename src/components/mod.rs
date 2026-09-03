use ratatui::widgets::Widget;

/// A rectangular area, wrapping ratatui's own. kept as a newtype so
/// ratatui never appears in a public signature.
#[derive(Clone, Copy)]
pub struct Rect(ratatui::layout::Rect);

/// A cell buffer, wrapping ratatui's own.
pub struct Buffer<'b>(&'b mut ratatui::buffer::Buffer);

impl From<Rect> for ratatui::layout::Rect {
  fn from(rect: Rect) -> Self {
    rect.0
  }
}

impl From<ratatui::layout::Rect> for Rect {
  fn from(rect: ratatui::layout::Rect) -> Self {
    Rect(rect)
  }
}

impl<'b> From<&'b mut ratatui::buffer::Buffer> for Buffer<'b> {
  fn from(buf: &'b mut ratatui::buffer::Buffer) -> Self {
    Buffer(buf)
  }
}

impl<'b> Buffer<'b> {
  pub(crate) fn inner_mut(&mut self) -> &mut ratatui::buffer::Buffer {
    &mut self.0
  }
}

/// What a `Component::view()` returns. Implemented for anything that's a
/// ratatui `Widget`, so built-in widgets get it for free without anyone
/// outside the crate importing ratatui.
pub trait Render {
  fn render(self, area: Rect, buf: &mut Buffer<'_>);
}

impl<W: Widget> Render for W {
  fn render(self, area: Rect, buf: &mut Buffer<'_>) {
    Widget::render(self, area.into(), buf.inner_mut());
  }
}

pub trait Component {
  fn view(&self) -> impl Render;
}
