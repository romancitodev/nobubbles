use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::{Paragraph, Widget};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
  components::Render,
  signals::{Signal, signal},
};

#[derive(Clone, Copy)]
pub struct Input {
  value: Signal<String>,
  cursor: Signal<usize>,
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
    }
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

  /// Trigger function to handle key events for the input. Returns true if the key event was handled, false otherwise.
  #[must_use]
  pub fn on_key(&self, key: KeyEvent) -> bool {
    match key.code {
      KeyCode::Char(c) => {
        let mut value = self.value.borrow_mut();
        let byte_idx = Self::byte_offset(&value, self.cursor.get());
        value.insert(byte_idx, c);
        self.cursor.update(|c| *c += 1);
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

impl Default for Input {
  fn default() -> Self {
    Self::new()
  }
}

impl Render for Input {
  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let value = self.value.get();
    Widget::render(Paragraph::new(value), area.into(), buf.inner_mut());
  }
}
