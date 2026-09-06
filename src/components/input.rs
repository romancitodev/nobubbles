use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
  text::Span,
  widgets::{Paragraph, Widget},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
  components::Render,
  signals::{Signal, signal},
  style::{Style, palette},
};

#[derive(Clone, Copy)]
pub struct Input {
  value: Signal<String>,
  cursor: Signal<usize>,
  /// The other end of a selection, grapheme index. `None` is "no selection", not "empty
  /// selection" — equal to `cursor` reads the same way and is treated as none too.
  anchor: Signal<Option<usize>>,
  multiline: Signal<bool>,
  masked: Signal<bool>,
  /// Even more paranoid than `masked`: draws nothing at all, not even the fixed mask.
  invisible: Signal<bool>,
  /// Shown, muted, only while the value is empty. Never part of the value — `&'static str`
  /// and not owned, since a hint is UI text set once at construction, not user data.
  placeholder: Option<&'static str>,
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
      anchor: signal(None),
      multiline: signal(false),
      masked: signal(false),
      invisible: signal(false),
      placeholder: None,
    }
  }

  /// Muted hint text, shown only while the field is empty — gone the moment anything is
  /// typed, and never part of [`Input::value`].
  #[must_use]
  pub fn placeholder(self, text: &'static str) -> Self {
    Self {
      placeholder: Some(text),
      ..self
    }
  }

  /// Draws a fixed-width mask instead of the text once there's anything typed, so a password
  /// never reaches the screen or the transcript. Fixed-width and not one dot per grapheme:
  /// matching the real length on screen is exactly the kind of leak masking is supposed to
  /// prevent (shoulder-surfing, screen recordings). Empty stays empty, so a placeholder can
  /// still show before anything's typed.
  #[must_use]
  pub fn masked(self) -> Self {
    self.masked.set(true);
    self
  }

  /// Past even `masked`: draws nothing at all, ever, mask included. For values where the
  /// fact that someone's typing something is itself worth hiding.
  #[must_use]
  pub fn invisible(self) -> Self {
    self.masked.set(true);
    self.invisible.set(true);
    self
  }

  const MASK_WIDTH: usize = 24;

  /// What to draw: the value, a mask that says nothing about how long it really is, or
  /// nothing at all.
  fn shown(&self) -> String {
    let value = self.value.get();
    if self.invisible.get() {
      return String::new();
    }
    if !self.masked.get() {
      return value;
    }
    if value.is_empty() {
      String::new()
    } else {
      "•".repeat(Self::MASK_WIDTH)
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

  /// Replaces the whole value, puts the cursor at the end, and drops any selection — for
  /// when something outside a keystroke decides what the field says, like a suggestion
  /// getting accepted.
  pub fn set(&self, text: impl Into<String>) {
    let text = text.into();
    self.cursor.set(Self::grapheme_count(&text));
    self.value.set(text);
    self.anchor.set(None);
  }

  /// Writes `c` at the cursor and steps past it.
  fn insert(&self, c: char) {
    let mut value = self.value.borrow_mut();
    let byte_idx = Self::byte_offset(&value, self.cursor.get());
    value.insert(byte_idx, c);
    drop(value);
    self.cursor.update(|at| *at += 1);
  }

  /// A grapheme counts as part of a word if its first scalar value is alphanumeric or `_` —
  /// the same class most editors use for ctrl+arrow and ctrl+backspace.
  fn is_word_char(grapheme: &str) -> bool {
    grapheme.chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_')
  }

  /// The grapheme index one word to the right of `cursor`: skip the run of separators the
  /// cursor is in front of, then skip the run of word characters after that — landing just
  /// past the next word, the way readline's `forward-word` does.
  ///
  /// Walked forward off the grapheme iterator directly rather than collected into a `Vec`
  /// first: unlike `prev_word`, nothing here needs to be indexed from the end.
  fn next_word(value: &str, cursor: usize) -> usize {
    let mut at = cursor;
    let mut graphemes = value.graphemes(true).skip(cursor).peekable();
    while graphemes.next_if(|&g| !Self::is_word_char(g)).is_some() {
      at += 1;
    }
    while graphemes.next_if(|&g| Self::is_word_char(g)).is_some() {
      at += 1;
    }
    at
  }

  /// The mirror of `next_word`, walking left: skip separators, then skip word characters,
  /// landing at the start of the previous word.
  fn prev_word(value: &str, cursor: usize) -> usize {
    let graphemes: Vec<&str> = value.graphemes(true).collect();
    let mut at = cursor;
    while at > 0 && !Self::is_word_char(graphemes[at - 1]) {
      at -= 1;
    }
    while at > 0 && Self::is_word_char(graphemes[at - 1]) {
      at -= 1;
    }
    at
  }

  /// Removes graphemes `[from, to)` and leaves the cursor at `from`. A no-op, not a panic,
  /// when the range is empty or backwards — callers hand in raw cursor/anchor pairs.
  fn delete_range(&self, from: usize, to: usize) {
    if from >= to {
      return;
    }
    let mut value = self.value.borrow_mut();
    let start = Self::byte_offset(&value, from);
    let end = Self::byte_offset(&value, to);
    value.replace_range(start..end, "");
    drop(value);
    self.cursor.set(from);
  }

  /// Deletes the selection, if there is one, and answers whether it did — so a caller like
  /// `Backspace` can fall back to its usual one-grapheme behaviour when there wasn't one.
  fn delete_selection(&self) -> bool {
    let cursor = self.cursor.get();
    let Some(anchor) = self.anchor.get().filter(|&a| a != cursor) else {
      self.anchor.set(None);
      return false;
    };
    self.delete_range(anchor.min(cursor), anchor.max(cursor));
    self.anchor.set(None);
    true
  }

  /// What typing a character does when a selection is live: the character replaces it,
  /// rather than landing next to it.
  fn replace_selection_with(&self, c: char) {
    self.delete_selection();
    self.insert(c);
  }

  /// If a real selection is live, collapses it to one edge and answers where — the near edge
  /// walking left, the far edge walking right. What a plain (unshifted) arrow does to a
  /// selection in most editors: the first press lands on the edge, it doesn't move past it.
  fn collapse_selection(&self, towards_end: bool) -> Option<usize> {
    let cursor = self.cursor.get();
    let anchor = self.anchor.get().filter(|&a| a != cursor)?;
    self.anchor.set(None);
    Some(if towards_end { anchor.max(cursor) } else { anchor.min(cursor) })
  }

  /// Moves the cursor to wherever `advance` says. With shift held, starts (or extends) a
  /// selection from the cursor's position before the move; without it, drops any selection.
  fn move_cursor(&self, shift: bool, advance: impl FnOnce(&str, usize) -> usize) {
    if shift {
      if self.anchor.get().is_none() {
        self.anchor.set(Some(self.cursor.get()));
      }
    } else {
      self.anchor.set(None);
    }

    let value = self.value.borrow();
    let next = advance(&value, self.cursor.get());
    drop(value);
    self.cursor.set(next);
  }

  /// Trigger function to handle key events for the input. Returns true if the key event was handled, false otherwise.
  #[must_use]
  pub fn on_key(&self, key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    // Left/Right/Backspace/Delete opt into ctrl (word-wise) and shift (selection); any other
    // ctrl combination belongs to whoever is driving — without this, ctrl+s types an `s`.
    if ctrl
      && !matches!(
        key.code,
        KeyCode::Left | KeyCode::Right | KeyCode::Backspace | KeyCode::Delete
      )
    {
      return false;
    }

    match key.code {
      KeyCode::Enter if self.multiline.get() => {
        self.delete_selection();
        self.insert('\n');
        true
      }
      KeyCode::Char(c) => {
        self.replace_selection_with(c);
        true
      }
      KeyCode::Backspace => {
        if self.delete_selection() {
          return true;
        }
        let cursor = self.cursor.get();
        if cursor == 0 {
          return true; // nothing before the cursor, but still an edit key
        }
        let from = if ctrl {
          Self::prev_word(&self.value.borrow(), cursor)
        } else {
          cursor - 1
        };
        self.delete_range(from, cursor);
        true
      }
      KeyCode::Delete => {
        if self.delete_selection() {
          return true;
        }
        let cursor = self.cursor.get();
        let value = self.value.borrow();
        let total = Self::grapheme_count(&value);
        if cursor >= total {
          return true; // nothing after the cursor
        }
        let to = if ctrl { Self::next_word(&value, cursor) } else { cursor + 1 };
        drop(value);
        self.delete_range(cursor, to);
        true
      }
      KeyCode::Left => {
        if !shift && let Some(at) = self.collapse_selection(false) {
          self.cursor.set(at);
        } else {
          self.move_cursor(shift, |value, cursor| {
            if ctrl { Self::prev_word(value, cursor) } else { cursor.saturating_sub(1) }
          });
        }
        true
      }
      KeyCode::Right => {
        if !shift && let Some(at) = self.collapse_selection(true) {
          self.cursor.set(at);
        } else {
          self.move_cursor(shift, |value, cursor| {
            if ctrl {
              Self::next_word(value, cursor)
            } else {
              (cursor + 1).min(Self::grapheme_count(value))
            }
          });
        }
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
    let cursor = self.cursor.get();

    // A masked field's caret would be the last leak left: its position tracks the real
    // length just as precisely as one dot per grapheme did. Nobody sets the cursor here, so
    // it draws hidden (see `Buffer::set_cursor`).
    if !self.masked.get() {
      let typed: String = value.graphemes(true).take(cursor).collect();

      // The row is how many newlines the cursor is past; the column is the width of what's
      // left on the current line. Columns and not graphemes, because a wide glyph takes two
      // cells and the caret has to clear both. `Span::width` is the measure ratatui lays the
      // text out with.
      let row = u16::try_from(typed.matches('\n').count()).unwrap_or(u16::MAX);
      let current = typed.rsplit('\n').next().unwrap_or("");
      let column = u16::try_from(Span::raw(current).width()).unwrap_or(u16::MAX);

      // ponytail: no horizontal scrolling, so a line wider than the area pins the caret at
      // the edge. Windowing the value is the same job as `Select::max_rows`, on the other axis.
      buf.set_cursor(
        area.x() + column.min(area.width().saturating_sub(1)),
        area.y() + row.min(area.height().saturating_sub(1)),
      );
    }

    let text: ratatui::text::Text = if value.is_empty()
      && let Some(placeholder) = self.placeholder
    {
      ratatui::text::Line::styled(placeholder, Style::new().fg(palette::OVERLAY0)).into()
    } else {
      match self.anchor.get().filter(|&a| a != cursor) {
        Some(anchor) => Self::highlighted(&value, anchor.min(cursor), anchor.max(cursor)),
        None => value.into(),
      }
    };
    Widget::render(Paragraph::new(text), area.into(), buf.inner_mut());
  }
}

impl Input {
  /// Splits `value` into lines and paints the graphemes in `[start, end)` — grapheme indices
  /// over the whole value, newlines counted — in reverse video, so a selection is visible.
  /// Built line by line and not as one `Span` with embedded `\n`s: a `Span` is one row, and a
  /// newline inside one wouldn't break the line the way it does in a plain string.
  fn highlighted(value: &str, start: usize, end: usize) -> ratatui::text::Text<'static> {
    let reversed = ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED);
    let mut at = 0;

    let lines = value
      .split('\n')
      .map(|line| {
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let mut spans = Vec::new();
        let mut plain = String::new();
        let mut selected = String::new();

        for (offset, grapheme) in graphemes.iter().enumerate() {
          if (start..end).contains(&(at + offset)) {
            if !plain.is_empty() {
              spans.push(Span::raw(std::mem::take(&mut plain)));
            }
            selected.push_str(grapheme);
          } else {
            if !selected.is_empty() {
              spans.push(Span::styled(std::mem::take(&mut selected), reversed));
            }
            plain.push_str(grapheme);
          }
        }
        if !plain.is_empty() {
          spans.push(Span::raw(plain));
        }
        if !selected.is_empty() {
          spans.push(Span::styled(selected, reversed));
        }

        at += graphemes.len() + 1; // +1 for the newline this line was split off of
        ratatui::text::Line::from(spans)
      })
      .collect::<Vec<_>>();

    ratatui::text::Text::from(lines)
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

  fn press_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
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

  /// The columns rendered in reverse video — the selection, if there is one.
  fn reversed_columns(field: Input) -> Vec<u16> {
    let area = RatatuiRect::new(0, 0, 20, 1);
    let mut raw = ratatui::buffer::Buffer::empty(area);
    let mut buf = Buffer::from(&mut raw);
    field.render(area.into(), &mut buf);

    (0..20)
      .filter(|&x| {
        raw[(x, 0)]
          .style()
          .add_modifier
          .contains(ratatui::style::Modifier::REVERSED)
      })
      .collect()
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

  /// The rendered row, cursor styling aside — just the text.
  fn rendered_text(field: Input, width: u16) -> String {
    let area = RatatuiRect::new(0, 0, width, 1);
    let mut raw = ratatui::buffer::Buffer::empty(area);
    let mut buf = Buffer::from(&mut raw);
    field.render(area.into(), &mut buf);

    (0..width)
      .map(|x| raw[(x, 0)].symbol().to_owned())
      .collect::<String>()
      .trim_end()
      .to_owned()
  }

  #[test]
  fn a_placeholder_shows_only_while_the_field_is_empty() {
    let field = Input::new().placeholder("your name");
    assert_eq!(rendered_text(field, 20), "your name");

    let _ = field.on_key(press(KeyCode::Char('x')));
    assert_eq!(rendered_text(field, 20), "x", "typing replaces it");
    assert_eq!(field.value(), "x", "and it was never the value to begin with");
  }

  #[test]
  fn masked_input_hides_the_real_length() {
    let field = Input::new().masked();
    assert_eq!(rendered_text(field, 20), "", "empty stays empty, so a placeholder can show");

    for c in "hi".chars() {
      let _ = field.on_key(press(KeyCode::Char(c)));
    }
    let short = rendered_text(field, 20);

    for c in "a much longer passphrase".chars() {
      let _ = field.on_key(press(KeyCode::Char(c)));
    }
    let long = rendered_text(field, 20);

    assert_eq!(short, long, "the mask must not grow with the real value");
    assert!(!short.is_empty());
  }

  /// A caret that moves with real keystrokes leaks the length just as precisely as one dot
  /// per grapheme would, even over a mask that never changes size.
  #[test]
  fn masked_input_never_shows_a_caret() {
    let field = Input::new().masked();
    assert_eq!(caret(field), None, "empty, nothing to hide, still no caret");

    for c in "some secret".chars() {
      let _ = field.on_key(press(KeyCode::Char(c)));
    }
    assert_eq!(caret(field), None);

    let _ = field.on_key(press(KeyCode::Left));
    let _ = field.on_key(press(KeyCode::Backspace));
    assert_eq!(caret(field), None, "no caret through edits either");
  }

  #[test]
  fn invisible_input_renders_nothing_but_still_holds_the_real_value() {
    let field = Input::new().invisible();
    assert_eq!(rendered_text(field, 20), "");
    assert_eq!(caret(field), None);

    for c in "a whole secret sentence".chars() {
      let _ = field.on_key(press(KeyCode::Char(c)));
    }

    assert_eq!(rendered_text(field, 20), "", "not even a mask shows up");
    assert_eq!(caret(field), None);
    assert_eq!(field.value(), "a whole secret sentence", "the real value still submits");
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

  #[test]
  fn ctrl_right_jumps_past_the_next_word() {
    let field = Input::with("one two three");
    assert!(field.on_key(press_mod(KeyCode::Right, KeyModifiers::CONTROL)));
    assert_eq!(caret(field), Some((6, 1)), "past \"one\"");

    assert!(field.on_key(press_mod(KeyCode::Right, KeyModifiers::CONTROL)));
    assert_eq!(caret(field), Some((10, 1)), "past \"two\", over the space between");
  }

  #[test]
  fn ctrl_left_jumps_to_the_start_of_the_previous_word() {
    let field = Input::with("one two three");
    for _ in 0..field.value().graphemes(true).count() {
      let _ = field.on_key(press(KeyCode::Right));
    }

    assert!(field.on_key(press_mod(KeyCode::Left, KeyModifiers::CONTROL)));
    assert_eq!(caret(field), Some((11, 1)), "start of \"three\"");

    assert!(field.on_key(press_mod(KeyCode::Left, KeyModifiers::CONTROL)));
    assert_eq!(caret(field), Some((7, 1)), "start of \"two\"");
  }

  #[test]
  fn ctrl_backspace_deletes_the_word_behind_the_cursor() {
    let field = Input::with("one two");
    for _ in 0..7 {
      let _ = field.on_key(press(KeyCode::Right));
    }

    assert!(field.on_key(press_mod(KeyCode::Backspace, KeyModifiers::CONTROL)));
    assert_eq!(field.value(), "one ", "\"two\" is gone, the space before it is not");
  }

  #[test]
  fn ctrl_delete_deletes_the_word_ahead_of_the_cursor() {
    let field = Input::with("one two");
    assert!(field.on_key(press_mod(KeyCode::Delete, KeyModifiers::CONTROL)));
    assert_eq!(field.value(), " two", "\"one\" is gone");
  }

  #[test]
  fn shift_right_selects_and_typing_replaces_the_selection() {
    let field = Input::with("cat");
    let _ = field.on_key(press_mod(KeyCode::Right, KeyModifiers::SHIFT));
    let _ = field.on_key(press_mod(KeyCode::Right, KeyModifiers::SHIFT));
    assert_eq!(reversed_columns(field), vec![0, 1], "\"ca\" is selected");

    let _ = field.on_key(press(KeyCode::Char('o')));
    assert_eq!(field.value(), "ot", "the selection is replaced, not kept around it");
    assert_eq!(reversed_columns(field), Vec::<u16>::new(), "and the selection is gone");
  }

  #[test]
  fn backspace_deletes_a_live_selection_instead_of_one_grapheme() {
    let field = Input::with("hello");
    for _ in 0..3 {
      let _ = field.on_key(press_mod(KeyCode::Right, KeyModifiers::SHIFT));
    }

    assert!(field.on_key(press(KeyCode::Backspace)));
    assert_eq!(field.value(), "lo", "the whole selection went, not just \"l\"");
  }

  #[test]
  fn ctrl_shift_right_selects_a_whole_word() {
    let field = Input::with("one two three");
    let _ = field.on_key(press_mod(
      KeyCode::Right,
      KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    ));

    assert_eq!(reversed_columns(field), vec![0, 1, 2], "\"one\" is selected");
    assert!(field.on_key(press(KeyCode::Backspace)));
    assert_eq!(field.value(), " two three");
  }

  /// A plain arrow collapses a selection to its edge instead of moving one further past it —
  /// the first press lands on the edge, the way it does in most editors.
  #[test]
  fn a_plain_arrow_collapses_the_selection_instead_of_moving_past_it() {
    let field = Input::with("hello");
    for _ in 0..3 {
      let _ = field.on_key(press_mod(KeyCode::Right, KeyModifiers::SHIFT));
    }

    assert!(field.on_key(press(KeyCode::Left)));
    assert_eq!(caret(field), Some((3, 1)), "back to the start of the selection");
    assert_eq!(reversed_columns(field), Vec::<u16>::new());
  }
}
