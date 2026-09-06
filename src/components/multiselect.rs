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
  /// First row of the window into the visible (filtered) rows. Only moves when the cursor
  /// would leave it.
  offset: Signal<usize>,
  style: Signal<MultiSelectStyle>,
  /// `/` opens this; empty means "not searching", not "searching for nothing".
  query: Signal<String>,
  searching: Signal<bool>,
  /// Whether [`MultiSelect::filter`] was called. `/` only opens search when it was — off by
  /// default, so an existing list of options that happens to include a literal `/` doesn't
  /// suddenly grow a mode nobody asked for.
  filterable: bool,
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
      query: signal(String::new()),
      searching: signal(false),
      filterable: false,
    }
  }

  /// Turns on `/` to search: typing narrows the list to what fuzzy-matches, keeping headings
  /// whose group still has a match. Arrows, space, and `ctrl+a` work the same as ever, over
  /// whatever's left visible.
  #[must_use]
  pub fn filter(self) -> Self {
    Self {
      filterable: true,
      ..self
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

  /// Whether row `at` is a heading. Out of range counts as one, so nothing lands there.
  fn is_header(&self, at: usize) -> bool {
    self
      .headers
      .with_ref(|headers| headers.get(at).copied().unwrap_or(true))
  }

  /// The absolute indices into `options` that pass the query, in their original order.
  ///
  /// Checked against a row's own text and its [`MultiSelect::note`], if it has one — a hint
  /// is content too, and hiding a row because the query matched the aside instead of the
  /// label would be strange. A heading is kept when at least one row under it still matches
  /// — filtered to nothing, a group heading with no options under it says nothing. One
  /// backward pass: a heading's fate depends on the rows *below* it, so walking from the end
  /// means each row is visited once, `group_matches` carrying whether anything in the group
  /// seen so far matched. Recomputed on every keystroke and every frame — cheap for the sizes
  /// a prompt list actually reaches; cache inside `MultiSelect` if a list ever gets big enough
  /// for that to matter.
  fn matches(&self) -> Vec<usize> {
    let query = self.query.get();
    if query.is_empty() {
      return (0..self.options.with_ref(Vec::len)).collect();
    }

    let matcher = SkimMatcherV2::default();
    let notes = self.notes.get();
    let mut group_matches = false;
    let mut out: Vec<usize> = self.headers.with_ref(|headers| {
      self.options.with_ref(|opts| {
        (0..opts.len())
          .rev()
          .filter(|&at| {
            if headers.get(at).copied().unwrap_or(false) {
              std::mem::take(&mut group_matches)
            } else {
              let hit =
                super::fuzzy_matches(&matcher, &query, &opts[at], notes.get(at).map_or("", |n| n));
              group_matches |= hit;
              hit
            }
          })
          .collect()
      })
    });
    out.reverse();
    out
  }

  /// The visible, tickable positions among `matches`: headings excluded, so the cursor only
  /// ever lands on something you can toggle. An iterator and not a `Vec` — every caller already
  /// has `matches` allocated; this just filters it in place.
  fn tickable<'a>(&'a self, matches: &'a [usize]) -> impl Iterator<Item = usize> + 'a {
    matches.iter().copied().filter(|&at| !self.is_header(at))
  }

  /// Walks the cursor by `by` positions among the tickable rows, wrapping.
  fn step(&self, by: i32) {
    let matches = self.matches();
    let tickable: Vec<usize> = self.tickable(&matches).collect();
    if tickable.is_empty() {
      return;
    }

    let at = tickable.iter().position(|&row| row == self.cursor.get());
    let next = match at {
      Some(pos) => (pos as i32 + by).rem_euclid(tickable.len() as i32) as usize,
      None => 0,
    };

    self.cursor.set(tickable[next]);
    self.scroll_into_view(&matches);
  }

  /// Ticks (or unticks) the row under the cursor. Space does this outright; while searching,
  /// Enter does too — the query already narrowed things down to one, and submitting the whole
  /// prompt on Enter would just kick you out to the next one instead of registering the pick.
  fn toggle_cursor(&self) {
    let at = self.cursor.get();
    if !self.is_header(at) {
      self.checked.update(|checked| {
        if let Some(row) = checked.get_mut(at) {
          *row = !*row;
        }
      });
    }
  }

  /// The cursor and the window both talk about a row that may no longer exist once the query
  /// changes, so both get fixed up in one place right after it does.
  fn after_filter_changed(&self) {
    let matches = self.matches();
    let tickable: Vec<usize> = self.tickable(&matches).collect();
    if !tickable.contains(&self.cursor.get()) {
      self.cursor.set(tickable.first().copied().unwrap_or(0));
    }
    self.offset.set(0);
    self.scroll_into_view(&matches);
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
  /// view dirty, and the loop would repaint forever. Takes `matches` rather than recomputing
  /// it: every caller already has it in hand.
  fn scroll_into_view(&self, matches: &[usize]) {
    let Some(rows) = self.style.get().max_rows.map(|rows| rows as usize) else {
      return;
    };
    // A query that matches nothing (or a list with no rows to begin with) leaves nothing to
    // scroll to, and the heading lookback below indexes into `matches` — empty would panic.
    if matches.is_empty() {
      return;
    }

    let len = matches.len();
    let cursor = matches
      .iter()
      .position(|&row| row == self.cursor.get())
      .unwrap_or(0);
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
    // under, which is the whole reason the groups exist. The heading is looked for among
    // `matches`, same axis as `cursor` and `offset`.
    let head = (0..=cursor).rev().find(|&pos| self.is_header(matches[pos]));
    if let Some(head) = head
      && cursor - head < rows
    {
      self.offset.update(|offset| *offset = (*offset).min(head));
    }
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

  /// Replaces the style.
  #[must_use]
  pub fn style(self, style: MultiSelectStyle) -> Self {
    self.style.set(style);
    self
  }

  /// Arrows move the cursor, space ticks the row under it, `/` opens search.
  pub fn on_key(&self, key: KeyEvent) -> bool {
    if self.searching.get() {
      return self.on_key_searching(key);
    }

    match key.code {
      KeyCode::Up => {
        self.step(-1);
        true
      }
      KeyCode::Down => {
        self.step(1);
        true
      }
      KeyCode::Char(' ') => {
        self.toggle_cursor();
        true
      }
      KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
        self.toggle_all();
        true
      }
      KeyCode::Char('/') if self.filterable => {
        self.searching.set(true);
        true
      }
      _ => false,
    }
  }

  /// `on_key`, while the search row is open. Typing narrows the list (space included, so a
  /// multi-word query works) instead of doing anything else, and Enter ticks the highlighted
  /// row rather than submitting the whole prompt — `ask` only reads a plain Enter as submit,
  /// so consuming it here is what keeps a search-then-pick from bouncing you to the next
  /// prompt.
  fn on_key_searching(&self, key: KeyEvent) -> bool {
    match key.code {
      KeyCode::Up => {
        self.step(-1);
        true
      }
      KeyCode::Down => {
        self.step(1);
        true
      }
      KeyCode::Enter => {
        self.toggle_cursor();
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
    match (self.searching.get(), self.filterable) {
      (true, _) => "type to filter · ↑↓ to move · esc to clear · enter to submit",
      (false, true) => "↑↓ to move · space to toggle · ctrl+a for all · enter to submit · / to search",
      (false, false) => "↑↓ to move · space to toggle · ctrl+a for all · enter to submit",
    }
  }
}

impl Render for MultiSelect {
  fn height(&self, _: u16) -> u16 {
    let matches = self.matches();
    let body = if matches.is_empty() { 1 } else { self.window(&matches).len() };
    (body + usize::from(self.searching.get())) as u16
  }

  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let cursor = self.cursor.get();
    let style = self.style.get();
    let checked = self.checked.get();
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
      self.options.with_ref(|options| {
        for &i in &matches[window.clone()] {
          let option = &options[i];

          if self.is_header(i) {
            lines.push(Line::styled(option.to_string(), style.header));
            continue;
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
          lines.push(Line::from(spans));
        }
      });
    }

    Widget::render(Paragraph::new(lines), area.into(), buf.inner_mut());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::from(code)
  }

  /// The rows actually on screen, markers included.
  fn wide(list: MultiSelect, rows: u16, width: u16) -> Vec<String> {
    use ratatui::backend::TestBackend;

    let mut terminal = ratatui::Terminal::new(TestBackend::new(width, rows)).unwrap();
    terminal
      .draw(|frame| {
        let area = frame.area().into();
        let mut buf = crate::components::Buffer::from(frame.buffer_mut());
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

  /// An empty list is a library-level state, not a bug for the app to guard against —
  /// `MultiSelect` says so itself instead of rendering nothing.
  #[test]
  fn an_empty_list_shows_a_placeholder_instead_of_rendering_nothing() {
    let list = MultiSelect::new(Vec::<String>::new()).max_rows(3);
    assert_eq!(list.height(20), 1);
    assert_eq!(wide(list, 1, 20), ["no options"]);
  }

  /// The exact crash this guards: a query matching nothing, with `max_rows` set, used to
  /// index `matches[0]` on an empty slice while pulling the window back up to a heading.
  #[test]
  fn a_query_matching_nothing_does_not_panic_the_scroll_math() {
    let list = MultiSelect::new(["bun", "deno"]).max_rows(1).filter();
    list.on_key(press(KeyCode::Char('/')));
    list.on_key(press(KeyCode::Char('z')));

    assert!(list.matches().is_empty());
    assert_eq!(wide(list, 2, 20), ["/z", "no matches"]);
  }

  /// Search is opt-in: without `.filter()`, `/` is just a character nobody's listening for.
  #[test]
  fn slash_does_nothing_without_filter() {
    let list = MultiSelect::new(["a/b", "c"]);
    assert!(!list.on_key(press(KeyCode::Char('/'))));
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
      list.window(&list.matches()).contains(&0),
      "Quality is in sight from the start"
    );

    // Down to `vitest`, which is under Testing. Two steps, because row 3 is a heading and
    // the cursor goes over it.
    for _ in 0..2 {
      list.on_key(press(KeyCode::Down));
    }
    assert_eq!(list.cursor(), 4);
    assert!(
      list.window(&list.matches()).contains(&3),
      "Testing came along with its options"
    );

    // Down to `lint-staged`, the last row, under Tooling.
    for _ in 0..3 {
      list.on_key(press(KeyCode::Down));
    }
    assert_eq!(list.cursor(), 8);
    assert!(list.window(&list.matches()).contains(&6), "and so did Tooling");

    // All the way back up to `eslint`, under Quality.
    for _ in 0..5 {
      list.on_key(press(KeyCode::Up));
    }
    assert_eq!(list.cursor(), 1);
    assert!(
      list.window(&list.matches()).contains(&0),
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
      list.window(&list.matches()),
      2..5,
      "scrolled, keeping a row of context below"
    );

    list.on_key(press(KeyCode::Up));
    list.on_key(press(KeyCode::Up));
    list.on_key(press(KeyCode::Up));
    list.on_key(press(KeyCode::Up));
    assert_eq!(list.cursor(), 9, "wrapped off the top");
    assert_eq!(list.window(&list.matches()), 7..10, "and the window went with it");
  }

  #[test]
  fn slash_opens_search_and_the_cursor_lands_on_the_match() {
    let list = MultiSelect::new(["bun", "deno", "node"]).filter();
    assert!(list.on_key(press(KeyCode::Char('/'))));
    for c in "den".chars() {
      assert!(list.on_key(press(KeyCode::Char(c))));
    }

    assert_eq!(list.matches(), vec![1]);
    assert_eq!(list.cursor(), 1);
  }

  /// A heading with nothing left under it says nothing, so it drops out with its group.
  #[test]
  fn a_header_survives_the_filter_only_if_a_child_still_matches() {
    let list = MultiSelect::new(["Frontend", "react", "svelte", "Backend", "axum", "django"])
      .headers([0, 3])
      .filter();
    list.on_key(press(KeyCode::Char('/')));
    for c in "svel".chars() {
      list.on_key(press(KeyCode::Char(c)));
    }

    assert_eq!(
      list.matches(),
      vec![0, 2],
      "Frontend survives because svelte matches; Backend does not"
    );
    assert_eq!(list.cursor(), 2);
  }

  #[test]
  fn arrows_skip_both_headers_and_non_matches_while_searching() {
    let list = MultiSelect::new(["Frontend", "react", "redux", "Backend", "axum", "reqwest"])
      .headers([0, 3])
      .filter();
    list.on_key(press(KeyCode::Char('/')));
    list.on_key(press(KeyCode::Char('r')));
    list.on_key(press(KeyCode::Char('e')));
    assert_eq!(list.cursor(), 1, "lands on react");

    list.on_key(press(KeyCode::Down));
    assert_eq!(list.cursor(), 2, "redux");

    list.on_key(press(KeyCode::Down));
    assert_eq!(list.cursor(), 5, "reqwest, hopping over Backend and axum");

    list.on_key(press(KeyCode::Down));
    assert_eq!(list.cursor(), 1, "wraps back to react");
  }

  /// The fix for the bug where hitting Enter on a search result bounced you to the next
  /// prompt instead of picking the thing you searched for.
  #[test]
  fn enter_ticks_the_highlighted_row_instead_of_submitting_while_searching() {
    let list = MultiSelect::new(["bun", "deno", "node"]).filter();
    list.on_key(press(KeyCode::Char('/')));
    list.on_key(press(KeyCode::Char('d')));
    list.on_key(press(KeyCode::Char('e')));

    assert!(list.on_key(press(KeyCode::Enter)), "consumed, not left for ask");
    assert_eq!(list.selected(), vec![1]);

    // Enter is still left alone once search is closed, so submit works as normal.
    list.on_key(press(KeyCode::Esc));
    assert!(!list.on_key(press(KeyCode::Enter)));
  }

  #[test]
  fn space_types_into_the_query_instead_of_toggling_while_searching() {
    let list = MultiSelect::new(["bun", "deno"]).filter();
    list.on_key(press(KeyCode::Char('/')));
    assert!(list.on_key(press(KeyCode::Char(' '))));

    assert_eq!(list.query.get(), " ");
    assert_eq!(list.selected(), Vec::<usize>::new(), "nothing ticked");
  }

  #[test]
  fn esc_clears_the_query_and_restores_the_full_list() {
    let list = MultiSelect::new(["bun", "deno", "node"]).filter();
    list.on_key(press(KeyCode::Char('/')));
    for c in "den".chars() {
      list.on_key(press(KeyCode::Char(c)));
    }

    assert!(list.on_key(press(KeyCode::Esc)));
    assert_eq!(list.matches(), vec![0, 1, 2]);
    assert_eq!(list.cursor(), 1, "cursor stays on deno");
  }
}
