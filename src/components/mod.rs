use ratatui::widgets::Widget;

/// A rectangular area, wrapping ratatui's own. kept as a newtype so
/// ratatui never appears in a public signature.
#[derive(Clone, Copy)]
pub struct Rect(ratatui::layout::Rect);

/// A cell buffer, wrapping ratatui's own.
pub struct Buffer<'b>(&'b mut ratatui::buffer::Buffer);

impl Rect {
  pub fn width(&self) -> u16 {
    self.0.width
  }

  pub fn height(&self) -> u16 {
    self.0.height
  }
}

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

  /// How many rows this needs to render without clipping, given a width. Defaults to
  /// one line; multi-line widgets override it so containers like `Column` can size to fit.
  fn height(&self, width: u16) -> u16 {
    let _ = width;
    1
  }
}

impl<W: Widget> Render for W {
  fn render(self, area: Rect, buf: &mut Buffer<'_>) {
    Widget::render(self, area.into(), buf.inner_mut());
  }
}

pub trait Component {
  fn view(&self) -> impl Render;
}

/// Object-safe counterpart of `Render`, so children can be boxed into a `Vec`.
/// `Render::render` takes `self` by value, which a `dyn Render` can't call directly.
trait ErasedRender {
  fn render_erased(self: Box<Self>, area: Rect, buf: &mut Buffer<'_>);
  fn height_erased(&self, width: u16) -> u16;
}

impl<T: Render> ErasedRender for T {
  fn render_erased(self: Box<Self>, area: Rect, buf: &mut Buffer<'_>) {
    (*self).render(area, buf);
  }

  fn height_erased(&self, width: u16) -> u16 {
    self.height(width)
  }
}

/// Stacks child widgets vertically, giving each exactly the rows it reports needing.
#[derive(Default)]
pub struct Column {
  children: Vec<Box<dyn ErasedRender>>,
}

impl Column {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn child(mut self, child: impl Render + 'static) -> Self {
    self.children.push(Box::new(child));
    self
  }
}

/// Builds a `Column` from a list of children, without chaining `.child()` by hand.
#[macro_export]
macro_rules! column {
  ($($child:expr),* $(,)?) => {
    $crate::components::Column::new()
      $(.child($child))*
  };
}

impl Render for Column {
  fn render(self, area: Rect, buf: &mut Buffer<'_>) {
    if self.children.is_empty() {
      return;
    }
    let width = area.width();
    let constraints: Vec<_> = self
      .children
      .iter()
      .map(|c| ratatui::layout::Constraint::Length(c.height_erased(width)))
      .collect();
    let areas = ratatui::layout::Layout::vertical(constraints).split(area.into());
    for (child, area) in self.children.into_iter().zip(areas.iter().copied()) {
      child.render_erased(area.into(), buf);
    }
  }

  fn height(&self, width: u16) -> u16 {
    self.children.iter().map(|c| c.height_erased(width)).sum()
  }
}

pub mod input;

#[cfg(test)]
mod tests {
  use ratatui::{backend::TestBackend, widgets::Paragraph, Terminal};

  use super::*;

  #[test]
  fn column_macro_expands_to_chained_children() {
    let backend = TestBackend::new(10, 4);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
      .draw(|frame| {
        let area = frame.area().into();
        let mut buf = Buffer::from(frame.buffer_mut());
        column![Paragraph::new("top"), Paragraph::new("bottom")].render(area, &mut buf);
      })
      .unwrap();

    terminal.backend().assert_buffer_lines([
      "top       ",
      "bottom    ",
      "          ",
      "          ",
    ]);
  }

  #[test]
  fn column_sizes_each_child_to_its_own_height() {
    let backend = TestBackend::new(10, 4);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
      .draw(|frame| {
        let area = frame.area().into();
        let mut buf = Buffer::from(frame.buffer_mut());
        Column::new()
          .child(Paragraph::new("top"))
          .child(Paragraph::new("bottom"))
          .render(area, &mut buf);
      })
      .unwrap();

    terminal.backend().assert_buffer_lines([
      "top       ",
      "bottom    ",
      "          ",
      "          ",
    ]);
  }
}
