use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fuzzy_matcher::skim::SkimMatcherV2;
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

/// The painted parts of a [`Select`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SelectStyle {
  pub active_symbol: &'static str,
  pub inactive_symbol: &'static str,
  /// The picked row's marker. Its label stays plain, so the eye lands on the marker.
  pub active: Style,
  /// Every other row, marker and label together.
  pub inactive: Style,
  /// The aside on the row under the cursor.
  pub note: Style,
  /// Most rows drawn at once. `None` draws the whole list, however long it is.
  pub max_rows: Option<u16>,
}

impl Default for SelectStyle {
  fn default() -> Self {
    Self {
      active_symbol: "●",
      inactive_symbol: "○",
      active: Style::new().fg(palette::GREEN),
      inactive: Style::new().fg(palette::OVERLAY1),
      note: Style::new().fg(palette::OVERLAY0).italic(),
      max_rows: None,
    }
  }
}

#[derive(Clone, Copy)]
pub struct Select {
  options: Signal<Vec<Cow<'static, str>>>,
  /// One per option, empty where there is none. Only the row under the cursor shows its own.
  notes: Signal<Vec<Cow<'static, str>>>,
  selected: Signal<usize>,
  /// First row of the window into the visible (filtered) rows. Only moves when the cursor
  /// would leave it.
  offset: Signal<usize>,
  style: Signal<SelectStyle>,
  /// `/` opens this; empty means "not searching", not "searching for nothing".
  query: Signal<String>,
  searching: Signal<bool>,
  /// Whether [`Select::filter`] was called. `/` only opens search when it was — off by
  /// default, so an existing list of options that happens to include a literal `/` doesn't
  /// suddenly grow a mode nobody asked for.
  filterable: bool,
}

impl Select {
  pub fn new(options: impl Iterator<Item: Into<Cow<'static, str>>>) -> Self {
    let options: Vec<Cow<'_, str>> = options.map(Into::into).collect();
    Self {
      options: signal(options),
      notes: signal(Vec::new()),
      selected: signal(0),
      offset: signal(0),
      style: signal(SelectStyle::default()),
      query: signal(String::new()),
      searching: signal(false),
      filterable: false,
    }
  }

  /// Turns on `/` to search: typing narrows the list to what fuzzy-matches, arrows and Enter
  /// work the same as ever, over whatever's left visible.
  #[must_use]
  pub fn filter(self) -> Self {
    Self {
      filterable: true,
      ..self
    }
  }

  /// Starts the cursor on `index` instead of the first row. Out of range lands on the first.
  #[must_use]
  pub fn initial(self, index: usize) -> Self {
    let len = self.options.with_ref(Vec::len);
    if index < len {
      self.selected.set(index);
      self.scroll_into_view(&self.matches());
    }
    self
  }

  /// Shows at most `rows` options at a time, scrolling to keep the cursor in view.
  ///
  /// Without it a long list makes the view as tall as the list, which for something like a
  /// gitmoji picker means a viewport taller than the terminal.
  #[must_use]
  pub fn max_rows(self, rows: u16) -> Self {
    self
      .style
      .update(|style| style.max_rows = Some(rows.max(1)));
    self
  }

  /// An aside for one option, shown only while the cursor is on it.
  ///
  /// ```text
  /// ● bun (recommended)
  /// ○ pnpm
  /// ```
  ///
  /// Only on the active row: a column of asides is a second list to read, and the point of
  /// the thing is to say something about the option you are looking at right now.
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

  /// Replaces the style.
  #[must_use]
  pub fn style(self, style: SelectStyle) -> Self {
    self.style.set(style);
    self
  }

  pub fn on_key(&self, key: KeyEvent) -> bool {
    if self.searching.get() {
      return self.on_key_searching(key);
    }

    match key.code {
      KeyCode::Up => {
        self.move_cursor(-1);
        true
      }
      KeyCode::Down => {
        self.move_cursor(1);
        true
      }
      KeyCode::Char('/') if self.filterable => {
        self.searching.set(true);
        true
      }
      _ => false,
    }
  }

  /// `on_key`, while the search row is open. Typing narrows the list instead of doing
  /// anything else, so the usual navigation keys stay the same but nothing else does.
  fn on_key_searching(&self, key: KeyEvent) -> bool {
    match key.code {
      KeyCode::Up => {
        self.move_cursor(-1);
        true
      }
      KeyCode::Down => {
        self.move_cursor(1);
        true
      }
      KeyCode::Esc => {
        self.searching.set(false);
        self.query.set(String::new());
        self.after_filter_changed();
        true
      }
      KeyCode::Backspace => {
        self.query.update(|query| {
          query.pop();
        });
        self.after_filter_changed();
        true
      }
      KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
        self.query.update(|query| query.push(c));
        self.after_filter_changed();
        true
      }
      _ => false,
    }
  }

  /// The absolute indices into `options` that pass the query, in their original order.
  ///
  /// Checked against the option's own text and its [`Select::note`], if it has one — a hint
  /// is content too, and hiding an option because the query matched the aside instead of the
  /// label would be strange. Filtering only, not re-ranking: a `Select` doesn't shuffle the
  /// list the way a bare fzf prompt does, it just hides what doesn't match. Recomputed on
  /// every keystroke and every frame — cheap for the sizes a prompt list actually reaches;
  /// cache inside `Select` if a list ever gets big enough for that to matter.
  fn matches(&self) -> Vec<usize> {
    let query = self.query.get();
    if query.is_empty() {
      return (0..self.options.with_ref(Vec::len)).collect();
    }

    let matcher = SkimMatcherV2::default();
    let notes = self.notes.get();
    self.options.with_ref(|opts| {
      opts
        .iter()
        .enumerate()
        .filter(|(at, option)| {
          super::fuzzy_matches(&matcher, &query, option, notes.get(*at).map_or("", |n| n))
        })
        .map(|(at, _)| at)
        .collect()
    })
  }

  /// Moves the cursor by `by` positions within the visible (filtered) rows, wrapping.
  fn move_cursor(&self, by: i32) {
    let matches = self.matches();
    if matches.is_empty() {
      return;
    }

    let at = matches
      .iter()
      .position(|&option| option == self.selected.get())
      .unwrap_or(0);
    let next = (at as i32 + by).rem_euclid(matches.len() as i32) as usize;

    self.selected.set(matches[next]);
    self.scroll_into_view(&matches);
  }

  /// The cursor and the window both talk about a row that may no longer exist once the query
  /// changes, so both get fixed up in one place right after it does.
  fn after_filter_changed(&self) {
    let matches = self.matches();
    if !matches.contains(&self.selected.get()) {
      self.selected.set(matches.first().copied().unwrap_or(0));
    }
    self.offset.set(0);
    self.scroll_into_view(&matches);
  }

  /// Moves the window the least it can to keep the cursor inside it.
  ///
  /// Called from `on_key` and never from `render`: writing a signal while drawing marks the
  /// view dirty, and the loop would repaint forever. Takes `matches` rather than recomputing
  /// it: every caller already has it in hand.
  fn scroll_into_view(&self, matches: &[usize]) {
    let Some(rows) = self.style.get().max_rows.map(|rows| rows as usize) else {
      return;
    };

    let len = matches.len();
    let cursor = matches
      .iter()
      .position(|&option| option == self.selected.get())
      .unwrap_or(0);
    let last_offset = len.saturating_sub(rows);

    self.offset.update(|offset| {
      // The margin is dropped rather than honoured when the window is too small to hold it.
      let margin = MARGIN.min(rows.saturating_sub(1) / 2);

      if cursor < offset.saturating_add(margin) {
        // Includes wrapping from the top: the cursor lands on the last row and the window
        // jumps down with it.
        *offset = cursor.saturating_sub(margin);
      } else if cursor + margin >= *offset + rows {
        *offset = (cursor + margin + 1).saturating_sub(rows);
      }
      *offset = (*offset).min(last_offset);
    });
  }

  /// The rows currently on screen, as a range into `matches`.
  fn window(&self, matches: &[usize]) -> std::ops::Range<usize> {
    let len = matches.len();
    let rows = self
      .style
      .get()
      .max_rows
      .map_or(len, |rows| (rows as usize).min(len));

    let offset = self.offset.get().min(len.saturating_sub(rows));
    offset..offset + rows
  }

  /// Rows the body takes, not counting the search line itself: the option window, or one row
  /// saying there's nothing to show — the query matched nothing, or the list was empty to
  /// start with.
  fn body_rows(&self, matches: &[usize]) -> usize {
    if matches.is_empty() {
      1
    } else {
      self.window(matches).len()
    }
  }

  pub fn selected(&self) -> usize {
    self.selected.get()
  }

  pub fn value(&self) -> Option<Cow<'static, str>> {
    let selected = self.selected.get();
    self.options.with_ref(|opts| opts.get(selected).cloned())
  }
}

impl crate::components::Ask for Select {
  fn answer(&self) -> String {
    self.value().map(Cow::into_owned).unwrap_or_default()
  }

  fn controls(&self) -> &'static str {
    match (self.searching.get(), self.filterable) {
      (true, _) => "type to filter · ↑↓ to move · esc to clear · enter to submit",
      (false, true) => "↑↓ to move · enter to submit · / to search",
      (false, false) => "↑↓ to move · enter to submit",
    }
  }
}

impl Render for Select {
  fn height(&self, _: u16) -> u16 {
    let matches = self.matches();
    (self.body_rows(&matches) + usize::from(self.searching.get())) as u16
  }

  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let selected = self.selected.get();
    let style = self.style.get();
    let matches = self.matches();
    let window = self.window(&matches);
    let notes = self.notes.get();

    let mut lines: Vec<Line> = Vec::new();
    if self.searching.get() {
      lines.push(super::render_search_row(&self.query.get(), area, buf));
    }

    if matches.is_empty() {
      let message = if self.searching.get() { "no matches" } else { "no options" };
      lines.push(Line::styled(
        message,
        Style::new().fg(palette::OVERLAY0).italic(),
      ));
    } else {
      self.options.with_ref(|opts| {
        for &i in &matches[window.clone()] {
          let option = &opts[i];
          lines.push(if i == selected {
            let mut spans = vec![
              Span::styled(style.active_symbol, style.active),
              Span::raw(format!(" {option}")),
            ];
            if let Some(note) = notes.get(i).filter(|note| !note.is_empty()) {
              spans.push(Span::styled(format!(" {note}"), style.note));
            }
            Line::from(spans)
          } else {
            Line::styled(
              format!("{} {option}", style.inactive_symbol),
              style.inactive,
            )
          });
        }
      });
    }

    Widget::render(Paragraph::new(lines), area.into(), buf.inner_mut());
  }
}

#[cfg(test)]
mod tests {
  use ratatui::backend::TestBackend;

  use super::*;
  use crate::components::Buffer;

  fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::from(code)
  }

  /// The rows actually on screen, markers included.
  fn visible(list: Select, rows: u16) -> Vec<String> {
    wide(list, rows, 12)
  }

  fn wide(list: Select, rows: u16, width: u16) -> Vec<String> {
    let mut terminal = ratatui::Terminal::new(TestBackend::new(width, rows)).unwrap();
    terminal
      .draw(|frame| {
        let area = frame.area().into();
        let mut buf = Buffer::from(frame.buffer_mut());
        list.render(area, &mut buf);
      })
      .unwrap();

    let buffer = terminal.backend().buffer().clone();
    (0..rows)
      .map(|y| {
        (0..width)
          .map(|x| buffer[(x, y)].symbol().to_owned())
          .collect::<String>()
          .trim_end()
          .to_owned()
      })
      .collect()
  }

  fn digits(count: usize) -> Select {
    Select::new((0..count).map(|n| n.to_string())).max_rows(3)
  }

  /// An empty list is a library-level state, not a bug for the app to guard against —
  /// `Select` says so itself instead of rendering nothing.
  #[test]
  fn an_empty_list_shows_a_placeholder_instead_of_rendering_nothing() {
    let list = Select::new(Vec::<String>::new().into_iter()).max_rows(3);
    assert_eq!(list.height(20), 1);
    assert_eq!(wide(list, 1, 20), ["no options"]);
  }

  /// Search is opt-in: without `.filter()`, `/` is just a character nobody's listening for.
  #[test]
  fn slash_does_nothing_without_filter() {
    let list = Select::new(["a/b", "c"].into_iter());
    assert!(!list.on_key(press(KeyCode::Char('/'))));
  }

  /// The aside rides the cursor: it belongs to the row you are on, not to the row it is on.
  #[test]
  fn a_note_shows_only_while_the_cursor_is_on_its_option() {
    let list = Select::new(["bun", "pnpm"].into_iter()).note(0, "(recommended)");

    assert_eq!(
      wide(list, 2, 20),
      ["● bun (recommended)", "○ pnpm"]
    );

    list.on_key(press(KeyCode::Down));
    assert_eq!(wide(list, 2, 20), ["○ bun", "● pnpm"]);
  }

  #[test]
  fn the_view_is_as_tall_as_the_window_not_the_list() {
    assert_eq!(digits(10).height(20), 3);
    assert_eq!(Select::new((0..10).map(|n| n.to_string())).height(20), 10);
    assert_eq!(
      digits(2).height(20),
      2,
      "a short list never pads out to the cap"
    );
  }

  /// The window keeps a row of context past the cursor, so the list moves before the cursor
  /// reaches the edge. Without that margin the cursor walks into the last visible row and
  /// only then does the list jump, which reads as the list moving rather than you moving.
  #[test]
  fn the_window_scrolls_a_row_before_the_cursor_reaches_the_edge() {
    let list = digits(10);
    assert_eq!(
      visible(list, 3),
      ["● 0", "○ 1", "○ 2"],
      "at the top there is no context"
    );

    list.on_key(press(KeyCode::Down));
    assert_eq!(
      visible(list, 3),
      ["○ 0", "● 1", "○ 2"],
      "a row of context below, so nothing has to move yet"
    );

    list.on_key(press(KeyCode::Down));
    assert_eq!(
      visible(list, 3),
      ["○ 1", "● 2", "○ 3"],
      "one more and the window follows, keeping the row below in sight"
    );
  }

  /// The wrap is where a naive window breaks: the cursor jumps to the end of the list and
  /// the window has to follow it all the way down.
  #[test]
  fn wrapping_off_the_top_takes_the_window_with_it() {
    let list = digits(10);

    list.on_key(press(KeyCode::Up));
    assert_eq!(list.selected(), 9);
    assert_eq!(visible(list, 3), ["○ 7", "○ 8", "● 9"]);

    list.on_key(press(KeyCode::Down));
    assert_eq!(list.selected(), 0);
    assert_eq!(
      visible(list, 3),
      ["● 0", "○ 1", "○ 2"],
      "and back to the top"
    );
  }

  /// The caret has to actually move, or a search row reads as a label instead of a field.
  #[test]
  fn the_caret_rides_the_end_of_the_query_while_searching() {
    let list = Select::new(["bun", "deno"].into_iter()).filter();
    list.on_key(press(KeyCode::Char('/')));
    list.on_key(press(KeyCode::Char('d')));
    list.on_key(press(KeyCode::Char('e')));

    let mut raw = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 20, 3));
    let mut buf = Buffer::from(&mut raw);
    list.render(ratatui::layout::Rect::new(0, 0, 20, 3).into(), &mut buf);

    assert_eq!(buf.cursor(), Some((3, 0)), "past \"/de\"");
  }

  #[test]
  fn slash_opens_search_and_typing_narrows_the_list() {
    let list = Select::new(["bun", "deno", "node"].into_iter()).filter();
    assert!(list.on_key(press(KeyCode::Char('/'))));

    for c in "den".chars() {
      assert!(list.on_key(press(KeyCode::Char(c))));
    }

    assert_eq!(wide(list, 2, 20), ["/den", "● deno"]);
    assert_eq!(list.value().as_deref(), Some("deno"));
  }

  /// A query can hit the note instead of the label — a hint is content too.
  #[test]
  fn a_query_can_match_the_note_instead_of_the_label() {
    let list = Select::new(["bun", "pnpm", "npm"].into_iter())
      .note(0, "(recommended)")
      .filter();
    list.on_key(press(KeyCode::Char('/')));
    for c in "recom".chars() {
      list.on_key(press(KeyCode::Char(c)));
    }

    assert_eq!(list.matches(), vec![0], "bun matched through its note");
  }

  #[test]
  fn a_query_with_no_matches_shows_a_placeholder_row() {
    let list = Select::new(["bun", "deno", "node"].into_iter()).filter();
    list.on_key(press(KeyCode::Char('/')));
    list.on_key(press(KeyCode::Char('z')));

    assert_eq!(list.height(20), 2);
    assert_eq!(wide(list, 2, 20), ["/z", "no matches"]);
  }

  #[test]
  fn esc_clears_the_query_and_restores_the_full_list() {
    let list = Select::new(["bun", "deno", "node"].into_iter()).filter();
    list.on_key(press(KeyCode::Char('/')));
    for c in "den".chars() {
      list.on_key(press(KeyCode::Char(c)));
    }

    assert!(list.on_key(press(KeyCode::Esc)));
    assert_eq!(
      wide(list, 3, 20),
      ["○ bun", "● deno", "○ node"],
      "back to the full list, cursor still on deno"
    );
  }

  /// `ask` reads an Enter the widget didn't want as "submit" (D-015/D-020) — search must not
  /// swallow it, or there would be no way to accept the highlighted match.
  #[test]
  fn enter_is_left_for_the_caller_even_while_searching() {
    let list = Select::new(["bun", "deno"].into_iter()).filter();
    list.on_key(press(KeyCode::Char('/')));
    assert!(!list.on_key(press(KeyCode::Enter)));
  }

  /// The cursor only ever lands on a visible row: hidden matches are skipped, not counted.
  #[test]
  fn arrows_move_between_matches_only_and_wrap() {
    let list = Select::new(["aaa", "bbb", "aab", "ccc"].into_iter()).filter();
    list.on_key(press(KeyCode::Char('/')));
    list.on_key(press(KeyCode::Char('a')));
    list.on_key(press(KeyCode::Char('a')));
    assert_eq!(wide(list, 3, 20), ["/aa", "● aaa", "○ aab"]);

    assert!(list.on_key(press(KeyCode::Down)));
    assert_eq!(list.selected(), 2, "bbb is hidden, so it is skipped");

    assert!(list.on_key(press(KeyCode::Down)));
    assert_eq!(list.selected(), 0, "wraps back to the first match");
  }

  #[test]
  fn backspace_widens_the_filter_back_out() {
    let list = Select::new(["bun", "deno", "node"].into_iter()).filter();
    list.on_key(press(KeyCode::Char('/')));
    for c in "den".chars() {
      list.on_key(press(KeyCode::Char(c)));
    }
    assert_eq!(list.value().as_deref(), Some("deno"));

    list.on_key(press(KeyCode::Backspace));
    list.on_key(press(KeyCode::Backspace));
    list.on_key(press(KeyCode::Backspace));
    assert_eq!(
      wide(list, 3, 20),
      ["/", "○ bun", "● deno"],
      "empty query is the full list again, cursor still on deno"
    );
  }
}
