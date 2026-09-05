use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
  text::Span,
  widgets::{Paragraph, Widget},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
  components::Render,
  signals::{Signal, signal},
};

#[derive(Clone, Copy)]
pub struct Input {
  value: Signal<String>,
  cursor: Signal<usize>,
  multiline: Signal<bool>,
  masked: Signal<bool>,
}

impl Input {
  /// An empty field.
  pub fn new() -> Self {
    Self::with("")
  }

  /// A field pre-filled with `initial`, cursor at the start.
  pub fn with(initial: impl Into<String>) -> Self {
    Self {
      value: signal(initial.into()),
      cursor: signal(0),
      multiline: signal(false),
      masked: signal(false),
    }
  }

  /// Draws a dot per grapheme instead of the text, and answers with dots too, so a password
  /// never reaches the screen or the transcript.
  #[must_use]
  pub fn masked(self) -> Self {
    self.masked.set(true);
    self
  }

  /// What to draw: the value, or one dot per grapheme.
  fn shown(&self) -> String {
    let value = self.value.get();
    if self.masked.get() {
      "•".repeat(value.graphemes(true).count())
    } else {
      value
    }
  }

  /// Lets Enter break the line instead of ending the prompt.
  ///
  /// Submitting then falls to `ctrl+s`, which every terminal delivers unambiguously.
  /// `ctrl+enter` would read better and is not an option: most terminals send it as a plain
  /// Enter, so the two would be impossible to tell apart.
  #[must_use]
  pub fn multiline(self) -> Self {
    self.multiline.set(true);
    self
  }

  /// Returns the number of graphemes in the given string.
  fn grapheme_count(value: &str) -> usize {
    value.graphemes(true).count()
  }

  /// Returns the byte offset of the grapheme at the given index.
  fn byte_offset(value: &str, grapheme: usize) -> usize {
    value
      .grapheme_indices(true)
      .nth(grapheme)
      .map_or(value.len(), |(i, _)| i)
  }

  /// Returns the current value of the input.
  #[must_use]
  pub fn value(&self) -> String {
    self.value.get()
  }

  /// Writes `c` at the cursor and steps past it.
  fn insert(&self, c: char) {
    let mut value = self.value.borrow_mut();
    let byte_idx = Self::byte_offset(&value, self.cursor.get());
    value.insert(byte_idx, c);
    drop(value);
    self.cursor.update(|at| *at += 1);
  }

  /// Trigger function to handle key events for the input. Returns true if the key event was handled, false otherwise.
  #[must_use]
  pub fn on_key(&self, key: KeyEvent) -> bool {
    // Control sequences belong to whoever is driving: without this, ctrl+s types an `s`.
    if key.modifiers.contains(KeyModifiers::CONTROL) {
      return false;
    }

    match key.code {
      KeyCode::Enter if self.multiline.get() => {
        self.insert('\n');
        true
      }
      KeyCode::Char(c) => {
        self.insert(c);
        true
      }
      KeyCode::Backspace => {
        let cursor = self.cursor.get();
        if cursor == 0 {
          return true; // nothing before the cursor, but still an edit key
        }
        let mut value = self.value.borrow_mut();
        let start = Self::byte_offset(&value, cursor - 1);
        let end = Self::byte_offset(&value, cursor);
        value.replace_range(start..end, "");
        self.cursor.update(|c| *c -= 1);
        true
      }
      KeyCode::Delete => {
        let mut value = self.value.borrow_mut();
        let cursor = self.cursor.get();
        if cursor >= Self::grapheme_count(&value) {
          return true; // nothing after the cursor
        }
        let start = Self::byte_offset(&value, cursor);
        let end = Self::byte_offset(&value, cursor + 1);
        value.replace_range(start..end, "");
        true
      }
      KeyCode::Left => {
        self.cursor.update(|c| *c = c.saturating_sub(1));
        true
      }
      KeyCode::Right => {
        let total = Self::grapheme_count(&self.value.borrow());
        self.cursor.update(|c| *c = (*c + 1).min(total));
        true
      }
      _ => false,
    }
  }
}

impl crate::components::Ask for Input {
  fn answer(&self) -> String {
    self.shown()
  }

  fn controls(&self) -> &'static str {
    if self.multiline.get() {
      "enter for a new line · ctrl+s to submit"
    } else {
      "enter to submit"
    }
  }
}

impl Default for Input {
  fn default() -> Self {
    Self::new()
  }
}

impl Render for Input {
  fn height(&self, _: u16) -> u16 {
    // `split` and not `lines`, because a trailing newline is a row you can still type on.
    let rows = self.shown().split('\n').count().max(1);
    u16::try_from(rows).unwrap_or(u16::MAX)
  }

  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let value = self.shown();
    let typed: String = value.graphemes(true).take(self.cursor.get()).collect();

    // The row is how many newlines the cursor is past; the column is the width of what's
    // left on the current line. Columns and not graphemes, because a wide glyph takes two
    // cells and the caret has to clear both. `Span::width` is the measure ratatui lays the
    // text out with.
    let row = u16::try_from(typed.matches('\n').count()).unwrap_or(u16::MAX);
    let current = typed.rsplit('\n').next().unwrap_or("");
    let column = u16::try_from(Span::raw(current).width()).unwrap_or(u16::MAX);

    // ponytail: no horizontal scrolling, so a line wider than the area pins the caret at the
    // edge. Windowing the value is the same job as `Select::max_rows`, on the other axis.
    buf.set_cursor(
      area.x() + column.min(area.width().saturating_sub(1)),
      area.y() + row.min(area.height().saturating_sub(1)),
    );

    Widget::render(Paragraph::new(value), area.into(), buf.inner_mut());
  }
}

#[cfg(test)]
mod tests {
  use ratatui::layout::Rect as RatatuiRect;

  use super::*;
  use crate::components::Buffer;

  fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::from(code)
  }

  /// Renders into a fixed area away from the origin, so the caret has to be placed relative
  /// to the area and not to the buffer.
  fn caret(field: Input) -> Option<(u16, u16)> {
    let area = RatatuiRect::new(3, 1, 10, 4);
    let mut raw = ratatui::buffer::Buffer::empty(area);
    let mut buf = Buffer::from(&mut raw);
    field.render(area.into(), &mut buf);
    buf.cursor()
  }

  #[test]
  fn the_caret_starts_where_the_text_does() {
    assert_eq!(caret(Input::with("hello")), Some((3, 1)));
  }

  /// Columns, not graphemes: a wide glyph takes two cells and the caret has to clear both.
  #[test]
  fn the_caret_counts_columns_not_graphemes() {
    let field = Input::with("日本");
    let _ = field.on_key(press(KeyCode::Right));
    assert_eq!(caret(field), Some((5, 1)), "one wide glyph is two columns");

    let _ = field.on_key(press(KeyCode::Right));
    assert_eq!(caret(field), Some((7, 1)));
  }

  #[test]
  fn typing_walks_the_caret_along() {
    let field = Input::new();
    let _ = field.on_key(press(KeyCode::Char('h')));
    let _ = field.on_key(press(KeyCode::Char('i')));
    assert_eq!(caret(field), Some((5, 1)));

    let _ = field.on_key(press(KeyCode::Backspace));
    assert_eq!(caret(field), Some((4, 1)));
  }

  #[test]
  fn plain_input_leaves_enter_alone_so_the_prompt_can_submit() {
    let field = Input::new();
    assert!(!field.on_key(press(KeyCode::Enter)));
    assert_eq!(field.value(), "");
  }

  #[test]
  fn multiline_takes_enter_and_grows_a_row() {
    let field = Input::new().multiline();
    assert_eq!(field.height(20), 1);

    let _ = field.on_key(press(KeyCode::Char('a')));
    assert!(field.on_key(press(KeyCode::Enter)), "multiline wants Enter");
    let _ = field.on_key(press(KeyCode::Char('b')));

    assert_eq!(
      field.value(),
      "a
b"
    );
    assert_eq!(field.height(20), 2);
    assert_eq!(caret(field), Some((4, 2)), "second row, one column in");
  }

  /// A trailing newline is a row you can still type on, which `str::lines` doesn't count.
  #[test]
  fn a_trailing_newline_is_still_a_row() {
    let field = Input::with(
      "a
",
    )
    .multiline();
    assert_eq!(field.height(20), 2);
  }

  #[test]
  fn control_keys_are_not_text() {
    let field = Input::new();
    let ctrl_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);

    assert!(
      !field.on_key(ctrl_s),
      "ctrl+s belongs to whoever is driving"
    );
    assert_eq!(field.value(), "", "and it is definitely not an `s`");
  }
}
