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
  /// Most rows drawn at once. `None` draws the whole list, however long it is.
  pub max_rows: Option<u16>,
}

impl Default for MultiSelectStyle {
  fn default() -> Self {
    Self {
      checked_symbol: "◼",
      unchecked_symbol: "◻",
      checked: Style::new().fg(Color::Green),
      cursor: Style::new().fg(Color::Cyan),
      inactive: Style::new().dim(),
      max_rows: None,
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
  /// First row of the window into `options`. Only moves when the cursor would leave it.
  offset: Signal<usize>,
  style: Signal<MultiSelectStyle>,
}

impl MultiSelect {
  pub fn new(options: impl IntoIterator<Item: Into<Cow<'static, str>>>) -> Self {
    let options: Vec<Cow<'static, str>> = options.into_iter().map(Into::into).collect();
    Self {
      checked: signal(vec![false; options.len()]),
      options: signal(options),
      cursor: signal(0),
      offset: signal(0),
      style: signal(MultiSelectStyle::default()),
    }
  }

  /// Shows at most `rows` options at a time, scrolling to keep the cursor in view.
  #[must_use]
  pub fn max_rows(self, rows: u16) -> Self {
    self
      .style
      .update(|style| style.max_rows = Some(rows.max(1)));
    self
  }

  /// Moves the window the least it can to keep the cursor inside it.
  ///
  /// Called from `on_key` and never from `render`: writing a signal while drawing marks the
  /// view dirty, and the loop would repaint forever.
  fn scroll_into_view(&self) {
    let Some(rows) = self.style.get().max_rows.map(|rows| rows as usize) else {
      return;
    };

    let len = self.options.with_ref(Vec::len);
    let cursor = self.cursor.get();
    let last_offset = len.saturating_sub(rows);

    self.offset.update(|offset| {
      if cursor < *offset {
        // Includes wrapping from the top: the cursor lands on the last row and the window
        // jumps down with it.
        *offset = cursor;
      } else if cursor >= *offset + rows {
        *offset = cursor + 1 - rows;
      }
      *offset = (*offset).min(last_offset);
    });
  }

  /// The rows currently on screen, as a range into `options`.
  fn window(&self) -> std::ops::Range<usize> {
    let len = self.options.with_ref(Vec::len);
    let rows = self
      .style
      .get()
      .max_rows
      .map_or(len, |rows| (rows as usize).min(len));

    let offset = self.offset.get().min(len.saturating_sub(rows));
    offset..offset + rows
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
          self.scroll_into_view();
        }
        true
      }
      KeyCode::Down => {
        let len = self.options.with_ref(Vec::len);
        if len > 0 {
          self.cursor.update(|at| *at = (*at + 1) % len);
          self.scroll_into_view();
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

impl crate::components::Ask for MultiSelect {
  fn answer(&self) -> String {
    let picked = self.values();
    if picked.is_empty() {
      "none".to_owned()
    } else {
      picked.join(", ")
    }
  }

  fn controls(&self) -> &'static str {
    "↑↓ to move · space to toggle · enter to submit"
  }
}

impl Render for MultiSelect {
  fn height(&self, _: u16) -> u16 {
    self.window().len() as u16
  }

  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let cursor = self.cursor.get();
    let style = self.style.get();
    let checked = self.checked.get();
    let window = self.window();

    let lines: Vec<Line> = self.options.with_ref(|options| {
      options
        .iter()
        .enumerate()
        .skip(window.start)
        .take(window.len())
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

  #[test]
  fn the_window_follows_the_cursor_around_the_wrap() {
    let list = MultiSelect::new((0..10).map(|n| n.to_string())).max_rows(3);
    assert_eq!(list.height(20), 3, "as tall as the window, not the list");

    for _ in 0..3 {
      list.on_key(press(KeyCode::Down));
    }
    assert_eq!(list.window(), 1..4, "scrolled by one, the least it could");

    list.on_key(press(KeyCode::Up));
    list.on_key(press(KeyCode::Up));
    list.on_key(press(KeyCode::Up));
    list.on_key(press(KeyCode::Up));
    assert_eq!(list.cursor(), 9, "wrapped off the top");
    assert_eq!(list.window(), 7..10, "and the window went with it");
  }
}
