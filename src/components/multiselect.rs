use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
  text::{Line, Span},
  widgets::{Paragraph, Widget},
};

use crate::{
  components::Render,
  signals::{Signal, signal},
  style::{Style, palette},
};

/// Rows of context kept past the cursor, so the list scrolls *before* the cursor reaches the
/// edge. Without it the cursor walks into the last visible row and the list only then jumps,
/// which reads as the list moving on its own rather than as you moving through it.
const MARGIN: usize = 1;

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
  /// A group heading, which is a row you can't tick.
  pub header: Style,
  /// The aside on the row under the cursor.
  pub note: Style,
  /// Most rows drawn at once. `None` draws the whole list, however long it is.
  pub max_rows: Option<u16>,
}

impl Default for MultiSelectStyle {
  fn default() -> Self {
    Self {
      checked_symbol: "◼",
      unchecked_symbol: "◻",
      checked: Style::new().fg(palette::GREEN),
      cursor: Style::new().fg(palette::MAUVE),
      inactive: Style::new().fg(palette::OVERLAY1),
      header: Style::new().fg(palette::LAVENDER).bold(),
      note: Style::new().fg(palette::OVERLAY0).italic(),
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
  /// Rows that are headings: the cursor jumps over them and space ignores them.
  headers: Signal<Vec<bool>>,
  /// One per option, empty where there is none. Only the row under the cursor shows its own.
  notes: Signal<Vec<Cow<'static, str>>>,
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
      headers: signal(vec![false; options.len()]),
      notes: signal(Vec::new()),
      options: signal(options),
      cursor: signal(0),
      offset: signal(0),
      style: signal(MultiSelectStyle::default()),
    }
  }

  /// An aside for one option, shown only while the cursor is on it.
  ///
  /// ```text
  /// ◻ bun (recommended)
  /// ◻ pnpm
  /// ```
  #[must_use]
  pub fn note(self, at: usize, text: impl Into<Cow<'static, str>>) -> Self {
    self.notes.update(|notes| {
      if notes.len() <= at {
        notes.resize(at + 1, Cow::Borrowed(""));
      }
      notes[at] = text.into();
    });
    self
  }

  /// Marks rows as headings: drawn without a box, skipped by the cursor, deaf to space.
  ///
  /// The cursor moves to the first row that isn't one, so a list that opens with a heading
  /// doesn't start on something you can't answer.
  #[must_use]
  pub fn headers(self, at: impl IntoIterator<Item = usize>) -> Self {
    self.headers.update(|headers| {
      for row in at {
        if let Some(header) = headers.get_mut(row) {
          *header = true;
        }
      }
    });

    if self.is_header(self.cursor.get()) {
      self.step(1);
    }
    self
  }

  /// The nearest heading at or above `at`, if the list has any.
  fn heading_above(&self, at: usize) -> Option<usize> {
    self.headers.with_ref(|headers| {
      (0..=at)
        .rev()
        .find(|&row| headers.get(row).copied().unwrap_or(false))
    })
  }

  /// Whether row `at` is a heading. Out of range counts as one, so nothing lands there.
  fn is_header(&self, at: usize) -> bool {
    self
      .headers
      .with_ref(|headers| headers.get(at).copied().unwrap_or(true))
  }

  /// Walks the cursor by `by`, wrapping, until it lands on something tickable.
  ///
  /// Bounded by the length: a list that is all headings would otherwise spin forever.
  fn step(&self, by: usize) {
    let len = self.options.with_ref(Vec::len);
    if len == 0 {
      return;
    }

    let mut at = self.cursor.get();
    for _ in 0..len {
      at = (at + by) % len;
      if !self.is_header(at) {
        self.cursor.set(at);
        self.scroll_into_view();
        return;
      }
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
      // The margin is dropped rather than honoured when the window is too small to hold it.
      let margin = MARGIN.min(rows.saturating_sub(1) / 2);

      if cursor < offset.saturating_add(margin) {
        *offset = cursor.saturating_sub(margin);
      } else if cursor + margin >= *offset + rows {
        *offset = (cursor + margin + 1).saturating_sub(rows);
      }
      *offset = (*offset).min(last_offset);
    });

    // Then pull the window back up to the heading, whenever the group fits inside it. A
    // window full of options with no heading over them says nothing about what they are
    // under, which is the whole reason the groups exist.
    if let Some(head) = self.heading_above(cursor)
      && cursor - head < rows
    {
      self.offset.update(|offset| *offset = (*offset).min(head));
    }
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
        self.step(len.saturating_sub(1));
        true
      }
      KeyCode::Down => {
        self.step(1);
        true
      }
      KeyCode::Char(' ') => {
        let at = self.cursor.get();
        if !self.is_header(at) {
          self.checked.update(|checked| {
            if let Some(row) = checked.get_mut(at) {
              *row = !*row;
            }
          });
        }
        true
      }
      KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
        self.toggle_all();
        true
      }
      _ => false,
    }
  }

  /// Ticks everything, or unticks it when everything is already ticked.
  ///
  /// Headings are left alone: they have no box. A list that is entirely headings has nothing
  /// to tick, so `all` starts true and the first press would untick nothing — harmless.
  fn toggle_all(&self) {
    let headers = self.headers.get();
    let all = self.checked.with_ref(|checked| {
      checked
        .iter()
        .enumerate()
        .filter(|(at, _)| !headers.get(*at).copied().unwrap_or(false))
        .all(|(_, ticked)| *ticked)
    });

    self.checked.update(|checked| {
      for (at, row) in checked.iter_mut().enumerate() {
        if !headers.get(at).copied().unwrap_or(false) {
          *row = !all;
        }
      }
    });
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

  /// The row indices that aren't headings, in order. What a caller maps its own list with.
  pub fn rows(&self) -> Vec<usize> {
    self.headers.with_ref(|headers| {
      headers
        .iter()
        .enumerate()
        .filter_map(|(i, header)| (!header).then_some(i))
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

/// Columns the collapsed answer gets: the terminal, less the rail and a little slack.
fn budget() -> u16 {
  crossterm::terminal::size()
    .map_or(80, |(columns, _)| columns)
    .saturating_sub(8)
}

/// As many picks as fit on one line, then how many were left out.
///
/// One always shows, even when it doesn't fit: a line reading only `+24 more` says nothing
/// about what was picked.
fn summarise(picked: &[Cow<'static, str>], budget: u16) -> String {
  let tail = crate::rimel::width_of(&format!(", +{} more", picked.len()));
  let mut width = 0;
  let mut shown = 0;

  for (at, item) in picked.iter().enumerate() {
    let step = crate::rimel::width_of(item) + if at == 0 { 0 } else { 2 };
    if at > 0 && width + step + tail > budget {
      break;
    }
    width += step;
    shown += 1;
  }

  if shown == picked.len() {
    return picked.join(", ");
  }
  format!(
    "{}, +{} more",
    picked[..shown].join(", "),
    picked.len() - shown
  )
}

impl crate::components::Ask for MultiSelect {
  fn answer(&self) -> String {
    let picked = self.values();
    if picked.is_empty() {
      "none".to_owned()
    } else {
      summarise(&picked, budget())
    }
  }

  fn controls(&self) -> &'static str {
    "↑↓ to move · space to toggle · ctrl+a for all · enter to submit"
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

    let notes = self.notes.get();

    let lines: Vec<Line> = self.options.with_ref(|options| {
      options
        .iter()
        .enumerate()
        .skip(window.start)
        .take(window.len())
        .map(|(i, option)| {
          if self.is_header(i) {
            return Line::styled(option.to_string(), style.header);
          }

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

          let mut spans = vec![Span::styled(symbol, box_style), label];
          if i == cursor
            && let Some(note) = notes.get(i).filter(|note| !note.is_empty())
          {
            spans.push(Span::styled(format!(" {note}"), style.note));
          }
          Line::from(spans)
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

  /// Ctrl+A is a toggle, not a one-way switch, and it never touches a heading.
  #[test]
  fn ctrl_a_ticks_everything_and_then_unticks_it() {
    let list = MultiSelect::new(["Quality", "eslint", "prettier"]).headers([0]);
    let all = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);

    assert!(list.on_key(all));
    assert_eq!(list.selected(), vec![1, 2]);

    assert!(list.on_key(all));
    assert_eq!(list.selected(), Vec::<usize>::new());
  }

  #[test]
  fn the_answer_keeps_what_fits_and_counts_the_rest() {
    let picked: Vec<Cow<'static, str>> = ["alpha", "beta", "gamma", "delta"]
      .into_iter()
      .map(Cow::Borrowed)
      .collect();

    assert_eq!(summarise(&picked, 80), "alpha, beta, gamma, delta");
    assert_eq!(summarise(&picked, 24), "alpha, beta, +2 more");
    // Under any budget at all, one pick survives.
    assert_eq!(summarise(&picked, 1), "alpha, +3 more");
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

  /// Headings are rows you can't land on, which is the whole reason they exist.
  #[test]
  fn the_cursor_jumps_over_headings() {
    let list = MultiSelect::new(["Frontend", "react", "svelte", "Backend", "axum"]).headers([0, 3]);

    assert_eq!(list.cursor(), 1, "it opens on the first tickable row");

    list.on_key(press(KeyCode::Down));
    assert_eq!(list.cursor(), 2);

    list.on_key(press(KeyCode::Down));
    assert_eq!(list.cursor(), 4, "row 3 is a heading, so it is skipped");

    list.on_key(press(KeyCode::Down));
    assert_eq!(list.cursor(), 1, "and it wraps past the first heading too");
  }

  #[test]
  fn space_does_nothing_on_a_heading() {
    let list = MultiSelect::new(["Group", "one"]).headers([0]);
    // Land on the heading the only way possible: by asking for it directly.
    list.on_key(press(KeyCode::Char(' ')));
    assert_eq!(
      list.selected(),
      vec![1],
      "the cursor was moved off it, so it ticked the option"
    );

    assert_eq!(list.rows(), vec![1], "and only the option counts as a row");
  }

  /// The heading of the group you are standing in stays on screen. Without this the window
  /// scrolls past it and you are left looking at options with nothing saying what they are
  /// under, which is the only reason the groups exist.
  #[test]
  fn the_group_heading_stays_in_view_while_you_are_under_it() {
    // 0 Quality, 1 eslint, 2 prettier, 3 Testing, 4 vitest, 5 playwright,
    // 6 Tooling, 7 husky, 8 lint-staged
    let rows = [
      "Quality",
      "eslint",
      "prettier",
      "Testing",
      "vitest",
      "playwright",
      "Tooling",
      "husky",
      "lint-staged",
    ];
    let list = MultiSelect::new(rows).max_rows(6).headers([0, 3, 6]);

    assert!(
      list.window().contains(&0),
      "Quality is in sight from the start"
    );

    // Down to `vitest`, which is under Testing. Two steps, because row 3 is a heading and
    // the cursor goes over it.
    for _ in 0..2 {
      list.on_key(press(KeyCode::Down));
    }
    assert_eq!(list.cursor(), 4);
    assert!(
      list.window().contains(&3),
      "Testing came along with its options"
    );

    // Down to `lint-staged`, the last row, under Tooling.
    for _ in 0..3 {
      list.on_key(press(KeyCode::Down));
    }
    assert_eq!(list.cursor(), 8);
    assert!(list.window().contains(&6), "and so did Tooling");

    // All the way back up to `eslint`, under Quality.
    for _ in 0..5 {
      list.on_key(press(KeyCode::Up));
    }
    assert_eq!(list.cursor(), 1);
    assert!(
      list.window().contains(&0),
      "Quality is back, not scrolled off"
    );
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
    assert_eq!(
      list.window(),
      2..5,
      "scrolled, keeping a row of context below"
    );

    list.on_key(press(KeyCode::Up));
    list.on_key(press(KeyCode::Up));
    list.on_key(press(KeyCode::Up));
    list.on_key(press(KeyCode::Up));
    assert_eq!(list.cursor(), 9, "wrapped off the top");
    assert_eq!(list.window(), 7..10, "and the window went with it");
  }
}
