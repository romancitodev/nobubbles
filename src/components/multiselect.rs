use std::borrow::Cow;

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

/// The painted parts of a [`MultiSelect`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MultiSelectStyle {
  pub checked_symbol: &'static str,
  pub unchecked_symbol: &'static str,
  /// A ticked box, wherever the cursor is.
  pub checked: Style,
  /// The row under the cursor, when it isn't ticked.
  pub cursor: Style,
  /// Everything else, box and label together.
  pub inactive: Style,
}

impl Default for MultiSelectStyle {
  fn default() -> Self {
    Self {
      checked_symbol: "◼",
      unchecked_symbol: "◻",
      checked: Style::new().fg(Color::Green),
      cursor: Style::new().fg(Color::Cyan),
      inactive: Style::new().dim(),
    }
  }
}

/// A list where any number of rows can be ticked.
///
/// ```text
/// ◼ react
/// ◻ svelte
/// ◻ vue
/// ```
///
/// Arrows move, space ticks. Submitting is the caller's call, same as [`Select`].
///
/// [`Select`]: crate::components::select::Select
#[derive(Clone, Copy)]
pub struct MultiSelect {
  options: Signal<Vec<Cow<'static, str>>>,
  checked: Signal<Vec<bool>>,
  cursor: Signal<usize>,
  style: Signal<MultiSelectStyle>,
}

impl MultiSelect {
  pub fn new(options: impl IntoIterator<Item: Into<Cow<'static, str>>>) -> Self {
    let options: Vec<Cow<'static, str>> = options.into_iter().map(Into::into).collect();
    Self {
      checked: signal(vec![false; options.len()]),
      options: signal(options),
      cursor: signal(0),
      style: signal(MultiSelectStyle::default()),
    }
  }

  /// Replaces the style.
  #[must_use]
  pub fn style(self, style: MultiSelectStyle) -> Self {
    self.style.set(style);
    self
  }

  /// Arrows move the cursor, space ticks the row under it.
  pub fn on_key(&self, key: KeyEvent) -> bool {
    match key.code {
      KeyCode::Up => {
        let len = self.options.with_ref(Vec::len);
        if len > 0 {
          self.cursor.update(|at| *at = (*at + len - 1) % len);
        }
        true
      }
      KeyCode::Down => {
        let len = self.options.with_ref(Vec::len);
        if len > 0 {
          self.cursor.update(|at| *at = (*at + 1) % len);
        }
        true
      }
      KeyCode::Char(' ') => {
        let at = self.cursor.get();
        self.checked.update(|checked| {
          if let Some(row) = checked.get_mut(at) {
            *row = !*row;
          }
        });
        true
      }
      _ => false,
    }
  }

  /// Where the cursor sits.
  pub fn cursor(&self) -> usize {
    self.cursor.get()
  }

  /// The indices that are ticked, in order. Indices and not the text, so the caller can map
  /// them back to whatever it actually has.
  pub fn selected(&self) -> Vec<usize> {
    self.checked.with_ref(|checked| {
      checked
        .iter()
        .enumerate()
        .filter_map(|(i, ticked)| ticked.then_some(i))
        .collect()
    })
  }

  /// The ticked options, for when the text is what you wanted after all.
  pub fn values(&self) -> Vec<Cow<'static, str>> {
    let picked = self.selected();
    self.options.with_ref(|options| {
      picked
        .iter()
        .filter_map(|&i| options.get(i).cloned())
        .collect()
    })
  }
}

impl Render for MultiSelect {
  fn height(&self, _: u16) -> u16 {
    self.options.with_ref(|options| options.len()) as u16
  }

  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let cursor = self.cursor.get();
    let style = self.style.get();
    let checked = self.checked.get();

    let lines: Vec<Line> = self.options.with_ref(|options| {
      options
        .iter()
        .enumerate()
        .map(|(i, option)| {
          let ticked = checked.get(i).copied().unwrap_or(false);
          let symbol = if ticked {
            style.checked_symbol
          } else {
            style.unchecked_symbol
          };

          // Ticked wins over the cursor: what's already in matters more than where you are.
          let box_style = match (ticked, i == cursor) {
            (true, _) => style.checked,
            (false, true) => style.cursor,
            (false, false) => style.inactive,
          };

          let label = if i == cursor {
            Span::raw(format!(" {option}"))
          } else {
            Span::styled(format!(" {option}"), style.inactive)
          };

          Line::from(vec![Span::styled(symbol, box_style), label])
        })
        .collect()
    });

    Widget::render(Paragraph::new(lines), area.into(), buf.inner_mut());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::from(code)
  }

  #[test]
  fn space_ticks_the_row_under_the_cursor() {
    let list = MultiSelect::new(["react", "svelte", "vue"]);
    assert_eq!(list.selected(), Vec::<usize>::new());

    assert!(list.on_key(press(KeyCode::Char(' '))));
    assert!(list.on_key(press(KeyCode::Down)));
    assert!(list.on_key(press(KeyCode::Down)));
    assert!(list.on_key(press(KeyCode::Char(' '))));

    assert_eq!(list.selected(), vec![0, 2]);
    assert_eq!(list.values(), vec!["react", "vue"]);
  }

  #[test]
  fn space_ticks_and_unticks() {
    let list = MultiSelect::new(["one"]);
    list.on_key(press(KeyCode::Char(' ')));
    list.on_key(press(KeyCode::Char(' ')));
    assert_eq!(list.selected(), Vec::<usize>::new());
  }

  #[test]
  fn the_cursor_wraps_both_ways() {
    let list = MultiSelect::new(["one", "two", "three"]);

    assert!(list.on_key(press(KeyCode::Up)));
    assert_eq!(list.cursor(), 2, "up from the top lands on the bottom");

    assert!(list.on_key(press(KeyCode::Down)));
    assert_eq!(list.cursor(), 0);
  }

  #[test]
  fn enter_is_left_for_the_caller() {
    let list = MultiSelect::new(["one"]);
    assert!(!list.on_key(press(KeyCode::Enter)));
  }
}
