use std::borrow::Cow;

use crate::{
  components::{Buffer, Rect, Render},
  rimel::Block,
  style::{Style, palette},
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
  /// Refused: the answer didn't pass, and the closer carries why.
  Error,
}

impl PromptState {
  /// The marker on the header line.
  fn marker(self) -> (&'static str, Style) {
    match self {
      Self::Active => ("◆", Style::new().fg(palette::MAUVE)),
      Self::Submitted => ("◇", Style::new().fg(palette::GREEN)),
      Self::Cancelled => ("■", Style::new().fg(palette::RED)),
      Self::Error => ("▲", Style::new().fg(palette::PEACH)),
    }
  }

  /// The rail itself.
  fn rail(self) -> Style {
    match self {
      Self::Active => Style::new().fg(palette::MAUVE),
      Self::Submitted => Style::new().fg(palette::SURFACE2),
      Self::Cancelled => Style::new().fg(palette::RED),
      Self::Error => Style::new().fg(palette::PEACH),
    }
  }

  /// What closes the block. An active prompt is the end of the rail so far; a submitted one
  /// keeps it going, which is what joins one prompt to the next.
  fn closer(self) -> &'static str {
    match self {
      Self::Submitted => BAR,
      Self::Active | Self::Cancelled | Self::Error => BAR_END,
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
  /// A block and not a string, so a title can carry its own colours.
  title: Block,
  body: R,
  hint: Option<Cow<'static, str>>,
}

impl<R> Prompt<R> {
  pub fn new(state: PromptState, title: impl Into<Block>, body: R) -> Self {
    Self {
      state,
      title: title.into(),
      body,
      hint: None,
    }
  }

  /// A line alongside the closer: the keys while the prompt is live, or why it was refused.
  ///
  /// Not shown once answered: there is nothing left to drive, and a transcript full of key
  /// hints reads as noise.
  #[must_use]
  pub fn hint(mut self, hint: impl Into<Cow<'static, str>>) -> Self {
    self.hint = Some(hint.into());
    self
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

      // Only the first row: a title is one line, and anything below it would land on the body.
      crate::rimel::keep_awake(&self.title);
      for (x, row, text, style) in self.title.runs() {
        if row == 0 {
          let style: ratatui::style::Style = style.into();
          raw.set_string(area.x + GUTTER + x, area.y, text, style);
        }
      }

      for row in 1..=body_height {
        if row < area.height {
          raw.set_string(area.x, area.y + row, BAR, rail);
        }
      }

      let closer = body_height + 1;
      if closer < area.height {
        raw.set_string(area.x, area.y + closer, self.state.closer(), rail);

        if let Some(hint) = self.hint.as_ref() {
          let style = match self.state {
            PromptState::Error => Style::new().fg(palette::PEACH),
            _ => Style::new().fg(palette::OVERLAY0),
          };

          if self.state != PromptState::Submitted {
            let style: ratatui::style::Style = style.into();
            raw.set_string(area.x + GUTTER, area.y + closer, hint.as_ref(), style);
          }
        }
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
  fn a_hint_rides_on_the_closer_while_the_prompt_is_active() {
    let active = Prompt::new(PromptState::Active, "Name", Text::new("bun")).hint("enter to submit");
    assert_eq!(
      draw(active, 24, 4),
      ["◆  Name", "│  bun", "└  enter to submit", ""]
    );

    let done =
      Prompt::new(PromptState::Submitted, "Name", Text::new("bun")).hint("enter to submit");
    assert_eq!(
      draw(done, 24, 4),
      ["◇  Name", "│  bun", "│", ""],
      "answered, so no keys left"
    );
  }

  /// End to end: a multiline `Input` inside the frame, after typing and a line break.
  #[test]
  fn a_multiline_input_grows_the_frame() {
    use crate::components::input::Input;
    use crossterm::event::{KeyCode, KeyEvent};

    let field = Input::new().multiline();
    for code in [KeyCode::Char('a'), KeyCode::Enter, KeyCode::Char('b')] {
      let _ = field.on_key(KeyEvent::from(code));
    }

    let prompt = Prompt::new(PromptState::Active, "Body", field);
    assert_eq!(prompt.height(20), 4, "header, two rows, closer");

    let mut terminal = ratatui::Terminal::new(TestBackend::new(20, 5)).unwrap();
    terminal
      .draw(|frame| {
        let area = frame.area().into();
        let mut buf = Buffer::from(frame.buffer_mut());
        prompt.render(area, &mut buf);
      })
      .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let lines: Vec<String> = (0..5)
      .map(|y| {
        (0..20)
          .map(|x| buffer[(x, y)].symbol().to_owned())
          .collect::<String>()
          .trim_end()
          .to_owned()
      })
      .collect();

    assert_eq!(lines, ["◆  Body", "│  a", "│  b", "└", ""]);
  }

  /// A refusal is not a cancel: the prompt stays live, turns yellow and says why.
  #[test]
  fn a_refused_prompt_carries_the_reason() {
    let refused = Prompt::new(PromptState::Error, "Title", Text::new("")).hint("required");

    assert_eq!(draw(refused, 20, 4), ["▲  Title", "│", "└  required", ""]);
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
