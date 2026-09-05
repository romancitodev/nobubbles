use ratatui::widgets::Widget;

pub mod confirm;
pub mod input;
pub mod multiselect;
pub mod progress;
pub mod prompt;
pub mod select;
pub mod text;

/// A rectangular area, wrapping ratatui's own. kept as a newtype so
/// ratatui never appears in a public signature.
#[derive(Clone, Copy)]
pub struct Rect(ratatui::layout::Rect);

/// A cell buffer, wrapping ratatui's own, plus where the terminal cursor should end up.
///
/// The cursor rides along because a widget is the only thing that knows where it goes, while
/// the real cursor is set on the frame, one level above `render`. Whoever built the buffer
/// reads it back afterwards and hands it up.
pub struct Buffer<'b> {
  inner: &'b mut ratatui::buffer::Buffer,
  cursor: Option<(u16, u16)>,
}

impl Rect {
  pub fn x(&self) -> u16 {
    self.0.x
  }

  pub fn y(&self) -> u16 {
    self.0.y
  }

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
    Buffer {
      inner: buf,
      cursor: None,
    }
  }
}

impl<'b> Buffer<'b> {
  pub(crate) fn inner_mut(&mut self) -> &mut ratatui::buffer::Buffer {
    self.inner
  }

  /// Asks for the terminal cursor to sit at `(x, y)`, in buffer coordinates.
  ///
  /// The last widget to call this wins, which is the point: exactly one thing on screen owns
  /// the cursor. A view where nobody calls it draws with the cursor hidden.
  pub fn set_cursor(&mut self, x: u16, y: u16) {
    self.cursor = Some((x, y));
  }

  pub(crate) fn cursor(&self) -> Option<(u16, u16)> {
    self.cursor
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

/// What the prompt runner needs from the widget it's asking with.
pub trait Ask {
  /// The one line this collapses to once it has been answered.
  ///
  /// A live `Select` is a list of options; an answered one is the option that won. Swapping
  /// them on submit is what makes the transcript read as answers instead of as a pile of
  /// lists.
  fn answer(&self) -> String;

  /// The keys this listens to, shown next to the closer while it's active.
  fn controls(&self) -> &'static str {
    "enter to submit"
  }
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

  #[must_use]
  pub fn child(mut self, child: impl Render + 'static) -> Self {
    self.children.push(Box::new(child));
    self
  }
}

/// N rows of the same thing, straight from an iterator: `rows.iter().copied().collect()`.
/// Use [`column!`] instead when the children are a fixed list of different types.
impl<R: Render + 'static> FromIterator<R> for Column {
  fn from_iter<I: IntoIterator<Item = R>>(children: I) -> Self {
    children.into_iter().fold(Self::new(), Self::child)
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

#[cfg(test)]
mod tests {
  use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};

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
  fn a_column_collects_from_an_iterator_of_rows() {
    let rows = ["top", "bottom"].map(Paragraph::new);
    let column: Column = rows.into_iter().collect();

    assert_eq!(column.height(10), 2);
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
