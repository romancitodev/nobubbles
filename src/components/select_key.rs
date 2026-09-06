use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
  text::{Line, Span},
  widgets::{Paragraph, Widget},
};

use crate::{
  components::Render,
  signals::{Signal, signal},
  style::{Style, palette},
};

/// The painted parts of a [`SelectKey`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SelectKeyStyle {
  /// The key itself, which is what the eye is looking for.
  pub key: Style,
  pub label: Style,
}

impl Default for SelectKeyStyle {
  fn default() -> Self {
    Self {
      key: Style::new().fg(palette::MAUVE).bold(),
      label: Style::default(),
    }
  }
}

/// A list you answer by pressing a letter.
///
/// ```text
/// y  yes, ship it
/// n  no, hold on
/// e  edit the message
/// ```
///
/// There is no cursor and no Enter: the key that chooses is the key that answers.
#[derive(Clone, Copy)]
pub struct SelectKey {
  options: Signal<Vec<(char, Cow<'static, str>)>>,
  picked: Signal<Option<usize>>,
  style: Signal<SelectKeyStyle>,
}

impl SelectKey {
  pub fn new(options: impl IntoIterator<Item = (char, impl Into<Cow<'static, str>>)>) -> Self {
    let options = options
      .into_iter()
      .map(|(key, label)| (key.to_ascii_lowercase(), label.into()))
      .collect();

    Self {
      options: signal(options),
      picked: signal(None),
      style: signal(SelectKeyStyle::default()),
    }
  }

  /// Replaces the style.
  #[must_use]
  pub fn style(self, style: SelectKeyStyle) -> Self {
    self.style.set(style);
    self
  }

  /// Case insensitive: `Y` and `y` are the same answer.
  pub fn on_key(&self, key: KeyEvent) -> bool {
    let KeyCode::Char(pressed) = key.code else {
      return false;
    };
    let pressed = pressed.to_ascii_lowercase();

    let found = self
      .options
      .with_ref(|options| options.iter().position(|(key, _)| *key == pressed));

    match found {
      Some(at) => {
        self.picked.set(Some(at));
        true
      }
      None => false,
    }
  }

  /// The index pressed, once something was.
  pub fn picked(&self) -> Option<usize> {
    self.picked.get()
  }
}

impl crate::components::Ask for SelectKey {
  fn answer(&self) -> String {
    let picked = self.picked.get();
    self.options.with_ref(|options| {
      picked
        .and_then(|at| options.get(at))
        .map(|(_, label)| label.to_string())
        .unwrap_or_default()
    })
  }

  fn controls(&self) -> &'static str {
    "press a key to answer"
  }

  /// The whole point: the key that chooses also ends the prompt.
  fn done(&self) -> bool {
    self.picked.get().is_some()
  }
}

impl Render for SelectKey {
  fn height(&self, _: u16) -> u16 {
    self.options.with_ref(|options| options.len()) as u16
  }

  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let style = self.style.get();

    let lines: Vec<Line> = self.options.with_ref(|options| {
      options
        .iter()
        .map(|(key, label)| {
          Line::from(vec![
            Span::styled(key.to_string(), style.key),
            Span::styled(format!("  {label}"), style.label),
          ])
        })
        .collect()
    });

    Widget::render(Paragraph::new(lines), area.into(), buf.inner_mut());
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::components::Ask;

  fn press(c: char) -> KeyEvent {
    KeyEvent::from(KeyCode::Char(c))
  }

  #[test]
  fn the_key_that_chooses_also_answers() {
    let pick = SelectKey::new([('y', "yes"), ('n', "no")]);
    assert!(!pick.done(), "nothing pressed yet");

    assert!(pick.on_key(press('N')), "case insensitive");
    assert_eq!(pick.picked(), Some(1));
    assert_eq!(pick.answer(), "no");
    assert!(pick.done(), "and that ends the prompt");
  }

  #[test]
  fn a_key_that_is_not_an_option_is_left_alone() {
    let pick = SelectKey::new([('y', "yes")]);

    assert!(!pick.on_key(press('z')));
    assert!(!pick.on_key(KeyEvent::from(KeyCode::Enter)));
    assert_eq!(pick.picked(), None);
  }
}
