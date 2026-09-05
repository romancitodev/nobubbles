//! Colors and text attributes, as nobubbles' own vocabulary.
//!
//! Not a newtype over ratatui's: this is the seed of the theme layer, and it converts at the
//! border like [`Rect`](crate::components::Rect) and [`Buffer`](crate::components::Buffer) do.
//! It carries only what a widget in this crate actually paints today.

/// A terminal color.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Color {
  /// Whatever the terminal's own default is.
  #[default]
  Reset,
  Black,
  Red,
  Green,
  Yellow,
  Blue,
  Magenta,
  Cyan,
  Gray,
  DarkGray,
  LightRed,
  LightGreen,
  LightYellow,
  LightBlue,
  LightMagenta,
  LightCyan,
  White,
  /// True color. Falls back to the nearest palette entry on terminals without it.
  Rgb(u8, u8, u8),
}

/// How a piece of text is painted.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Style {
  pub fg: Color,
  pub bg: Color,
  pub bold: bool,
  pub dim: bool,
  pub italic: bool,
}

impl Style {
  /// A style that changes nothing.
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use]
  pub fn fg(mut self, color: Color) -> Self {
    self.fg = color;
    self
  }

  #[must_use]
  pub fn bg(mut self, color: Color) -> Self {
    self.bg = color;
    self
  }

  #[must_use]
  pub fn bold(mut self) -> Self {
    self.bold = true;
    self
  }

  #[must_use]
  pub fn dim(mut self) -> Self {
    self.dim = true;
    self
  }

  #[must_use]
  pub fn italic(mut self) -> Self {
    self.italic = true;
    self
  }
}

impl From<Color> for ratatui::style::Color {
  fn from(color: Color) -> Self {
    match color {
      Color::Reset => Self::Reset,
      Color::Black => Self::Black,
      Color::Red => Self::Red,
      Color::Green => Self::Green,
      Color::Yellow => Self::Yellow,
      Color::Blue => Self::Blue,
      Color::Magenta => Self::Magenta,
      Color::Cyan => Self::Cyan,
      Color::Gray => Self::Gray,
      Color::DarkGray => Self::DarkGray,
      Color::LightRed => Self::LightRed,
      Color::LightGreen => Self::LightGreen,
      Color::LightYellow => Self::LightYellow,
      Color::LightBlue => Self::LightBlue,
      Color::LightMagenta => Self::LightMagenta,
      Color::LightCyan => Self::LightCyan,
      Color::White => Self::White,
      Color::Rgb(r, g, b) => Self::Rgb(r, g, b),
    }
  }
}

impl From<Style> for ratatui::style::Style {
  fn from(style: Style) -> Self {
    use ratatui::style::Modifier;

    let mut modifier = Modifier::empty();
    modifier.set(Modifier::BOLD, style.bold);
    modifier.set(Modifier::DIM, style.dim);
    modifier.set(Modifier::ITALIC, style.italic);

    Self::default()
      .fg(style.fg.into())
      .bg(style.bg.into())
      .add_modifier(modifier)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn builders_stack_onto_one_style() {
    let style = Style::new().fg(Color::Rgb(1, 2, 3)).bold().dim().italic();

    let converted: ratatui::style::Style = style.into();
    assert_eq!(converted.fg, Some(ratatui::style::Color::Rgb(1, 2, 3)));
    assert!(converted.add_modifier.contains(
      ratatui::style::Modifier::BOLD
        | ratatui::style::Modifier::DIM
        | ratatui::style::Modifier::ITALIC
    ));
  }
}
