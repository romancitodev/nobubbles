use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
  text::{Line, Span},
  widgets::{Paragraph, Widget},
};

use crate::{
  components::Render,
  signals::{Signal, signal},
  style::{Color, Style},
};

/// The painted parts of a [`Confirm`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConfirmStyle {
  pub active_symbol: &'static str,
  pub inactive_symbol: &'static str,
  pub divider: &'static str,
  /// The picked side's marker. Its label stays plain.
  pub active: Style,
  /// The other side, and the divider.
  pub inactive: Style,
}

impl Default for ConfirmStyle {
  fn default() -> Self {
    Self {
      active_symbol: "●",
      inactive_symbol: "○",
      divider: " / ",
      active: Style::new().fg(Color::Green),
      inactive: Style::new().dim(),
    }
  }
}

/// A yes or no, on one row.
///
/// ```text
/// ● Yes / ○ No
/// ```
#[derive(Clone, Copy)]
pub struct Confirm {
  value: Signal<bool>,
  style: Signal<ConfirmStyle>,
}

impl Confirm {
  /// Starts on yes, which is what a confirm is usually for.
  pub fn new() -> Self {
    Self::with(true)
  }

  pub fn with(initial: bool) -> Self {
    Self {
      value: signal(initial),
      style: signal(ConfirmStyle::default()),
    }
  }

  /// Replaces the style.
  #[must_use]
  pub fn style(self, style: ConfirmStyle) -> Self {
    self.style.set(style);
    self
  }

  pub fn value(&self) -> bool {
    self.value.get()
  }

  /// Arrows and `hjkl` flip the answer; `y` and `n` say it outright.
  pub fn on_key(&self, key: KeyEvent) -> bool {
    match key.code {
      KeyCode::Left
      | KeyCode::Right
      | KeyCode::Up
      | KeyCode::Down
      | KeyCode::Char('h' | 'j' | 'k' | 'l') => {
        self.value.update(|value| *value = !*value);
        true
      }
      KeyCode::Char('y' | 'Y') => {
        self.value.set(true);
        true
      }
      KeyCode::Char('n' | 'N') => {
        self.value.set(false);
        true
      }
      _ => false,
    }
  }
}

impl crate::components::Ask for Confirm {
  fn answer(&self) -> String {
    if self.value() { "Yes" } else { "No" }.to_owned()
  }

  fn controls(&self) -> &'static str {
    "←→ or y/n · enter to submit"
  }
}

impl Default for Confirm {
  fn default() -> Self {
    Self::new()
  }
}

impl Render for Confirm {
  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let style = self.style.get();

    let side = |label: &'static str, picked: bool| {
      if picked {
        vec![
          Span::styled(style.active_symbol, style.active),
          Span::raw(format!(" {label}")),
        ]
      } else {
        vec![Span::styled(
          format!("{} {label}", style.inactive_symbol),
          style.inactive,
        )]
      }
    };

    let yes = self.value.get();
    let mut spans = side("Yes", yes);
    spans.push(Span::styled(style.divider, style.inactive));
    spans.extend(side("No", !yes));

    Widget::render(
      Paragraph::new(Line::from(spans)),
      area.into(),
      buf.inner_mut(),
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::from(code)
  }

  #[test]
  fn arrows_flip_and_letters_answer_outright() {
    let confirm = Confirm::new();
    assert!(confirm.value(), "a confirm starts on yes");

    assert!(confirm.on_key(press(KeyCode::Left)));
    assert!(!confirm.value());

    assert!(confirm.on_key(press(KeyCode::Char('l'))));
    assert!(confirm.value(), "any arrow flips, there are only two sides");

    assert!(confirm.on_key(press(KeyCode::Char('n'))));
    assert!(!confirm.value());
    assert!(
      confirm.on_key(press(KeyCode::Char('n'))),
      "saying no twice is still no"
    );
    assert!(!confirm.value());

    assert!(confirm.on_key(press(KeyCode::Char('Y'))));
    assert!(confirm.value());
  }

  #[test]
  fn enter_is_left_for_the_caller() {
    let confirm = Confirm::new();
    assert!(!confirm.on_key(press(KeyCode::Enter)));
    assert!(!confirm.on_key(press(KeyCode::Char('q'))));
  }
}
