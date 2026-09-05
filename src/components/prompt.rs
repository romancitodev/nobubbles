use std::borrow::Cow;

use crate::{
  components::{Buffer, Rect, Render},
  style::{Color, Style},
};

/// Columns the rail takes before the content starts.
const GUTTER: u16 = 3;

pub(crate) const BAR: &str = "│";
pub(crate) const BAR_START: &str = "┌";
pub(crate) const BAR_END: &str = "└";

/// Where a prompt is in its life. It picks the marker and the colour of the rail.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PromptState {
  #[default]
  Active,
  Submitted,
  Cancelled,
}

impl PromptState {
  /// The marker on the header line.
  fn marker(self) -> (&'static str, Style) {
    match self {
      Self::Active => ("◆", Style::new().fg(Color::Cyan)),
      Self::Submitted => ("◇", Style::new().fg(Color::Green)),
      Self::Cancelled => ("■", Style::new().fg(Color::Red)),
    }
  }

  /// The rail itself.
  fn rail(self) -> Style {
    match self {
      Self::Active => Style::new().fg(Color::Cyan),
      Self::Submitted => Style::new().dim(),
      Self::Cancelled => Style::new().fg(Color::Red),
    }
  }

  /// What closes the block. An active prompt is the end of the rail so far; a submitted one
  /// keeps it going, which is what joins one prompt to the next.
  fn closer(self) -> &'static str {
    match self {
      Self::Submitted => BAR,
      Self::Active | Self::Cancelled => BAR_END,
    }
  }
}

/// A prompt on the rail: a marker, a title, and a body indented underneath.
///
/// ```text
/// ◆  Project name
/// │  my-app
/// └
/// ```
pub struct Prompt<R> {
  state: PromptState,
  title: Cow<'static, str>,
  body: R,
}

impl<R> Prompt<R> {
  pub fn new(state: PromptState, title: impl Into<Cow<'static, str>>, body: R) -> Self {
    Self {
      state,
      title: title.into(),
      body,
    }
  }
}

impl<R: Render> Render for Prompt<R> {
  /// The header, the body, and the closing rail.
  fn height(&self, width: u16) -> u16 {
    2 + self.body.height(width.saturating_sub(GUTTER))
  }

  fn render(self, area: Rect, buf: &mut Buffer<'_>) {
    let content_width = area.width().saturating_sub(GUTTER);
    let area: ratatui::layout::Rect = area.into();
    if area.height == 0 {
      return;
    }

    let body_height = self.body.height(content_width);
    let (marker, marker_style) = self.state.marker();
    let marker_style: ratatui::style::Style = marker_style.into();
    let rail: ratatui::style::Style = self.state.rail().into();

    {
      // Everything on the rail is painted by hand: it's one column of glyphs, not a widget.
      let raw = buf.inner_mut();
      raw.set_string(area.x, area.y, marker, marker_style);
      raw.set_string(
        area.x + GUTTER,
        area.y,
        self.title.as_ref(),
        ratatui::style::Style::default(),
      );

      for row in 1..=body_height {
        if row < area.height {
          raw.set_string(area.x, area.y + row, BAR, rail);
        }
      }

      let closer = body_height + 1;
      if closer < area.height {
        raw.set_string(area.x, area.y + closer, self.state.closer(), rail);
      }
    }

    let body = ratatui::layout::Rect {
      x: area.x + GUTTER,
      y: area.y + 1,
      width: content_width,
      height: body_height.min(area.height.saturating_sub(1)),
    };
    self.body.render(body.into(), buf);
  }
}

#[cfg(test)]
mod tests {
  use ratatui::backend::TestBackend;

  use super::*;
  use crate::components::text::Text;

  fn draw(prompt: Prompt<Text>, width: u16, height: u16) -> Vec<String> {
    let mut terminal = ratatui::Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
      .draw(|frame| {
        let area = frame.area().into();
        let mut buf = Buffer::from(frame.buffer_mut());
        prompt.render(area, &mut buf);
      })
      .unwrap();

    let buffer = terminal.backend().buffer().clone();
    (0..height)
      .map(|y| {
        (0..width)
          .map(|x| buffer[(x, y)].symbol().to_owned())
          .collect::<String>()
          .trim_end()
          .to_owned()
      })
      .collect()
  }

  #[test]
  fn an_active_prompt_closes_the_rail() {
    let prompt = Prompt::new(PromptState::Active, "Name", Text::new("bun"));

    assert_eq!(prompt.height(20), 3);
    assert_eq!(draw(prompt, 20, 4), ["◆  Name", "│  bun", "└", ""]);
  }

  /// The difference that chains one prompt to the next: submitted keeps the rail going.
  #[test]
  fn a_submitted_prompt_keeps_the_rail_going() {
    let prompt = Prompt::new(PromptState::Submitted, "Name", Text::new("bun"));

    assert_eq!(draw(prompt, 20, 4), ["◇  Name", "│  bun", "│", ""]);
  }

  #[test]
  fn a_multi_row_body_gets_a_rail_per_row() {
    let prompt = Prompt::new(PromptState::Active, "Pick", Text::new("one\ntwo\nthree"));

    assert_eq!(prompt.height(20), 5);
    assert_eq!(
      draw(prompt, 20, 6),
      ["◆  Pick", "│  one", "│  two", "│  three", "└", ""]
    );
  }
}
