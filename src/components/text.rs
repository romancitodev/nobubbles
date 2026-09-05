use std::borrow::Cow;

use ratatui::widgets::{Paragraph, Widget};

use crate::{components::Render, style::Style};

/// A run of text, painted as one piece.
///
/// The plain building block for a row: a prompt, a label, a footer. It's what keeps an app
/// from reaching for a ratatui widget to put a string on screen.
pub struct Text {
  content: Cow<'static, str>,
  style: Style,
}

impl Text {
  pub fn new(content: impl Into<Cow<'static, str>>) -> Self {
    Self {
      content: content.into(),
      style: Style::default(),
    }
  }

  /// Paints it with `style` instead of the terminal's defaults.
  #[must_use]
  pub fn style(mut self, style: Style) -> Self {
    self.style = style;
    self
  }
}

impl Render for Text {
  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let paragraph = Paragraph::new(self.content).style(self.style);
    Widget::render(paragraph, area.into(), buf.inner_mut());
  }

  /// Nothing wraps yet, so the rows are exactly the newlines. An empty string still takes
  /// one, which is why this never returns zero.
  fn height(&self, _: u16) -> u16 {
    let lines = self.content.lines().count().max(1);
    u16::try_from(lines).unwrap_or(u16::MAX)
  }
}

#[cfg(test)]
mod tests {
  use ratatui::backend::TestBackend;

  use super::*;
  use crate::components::Buffer;
  use crate::style::Color;

  #[test]
  fn height_counts_the_lines_it_will_take() {
    assert_eq!(Text::new("").height(10), 1, "an empty row is still a row");
    assert_eq!(Text::new("one").height(10), 1);
    assert_eq!(Text::new("one\ntwo\nthree").height(10), 3);
  }

  #[test]
  fn the_style_reaches_the_cells() {
    let mut terminal = ratatui::Terminal::new(TestBackend::new(4, 1)).unwrap();
    let text = Text::new("done").style(Style::new().fg(Color::LightGreen).italic());

    terminal
      .draw(|frame| {
        let area = frame.area().into();
        let mut buf = Buffer::from(frame.buffer_mut());
        text.render(area, &mut buf);
      })
      .unwrap();

    let painted = terminal.backend().buffer()[(0, 0)].clone();
    assert_eq!(painted.symbol(), "d");
    assert_eq!(painted.fg, ratatui::style::Color::LightGreen);
    assert!(painted.modifier.contains(ratatui::style::Modifier::ITALIC));
  }
}
