use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent};
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{
  text::{Line, Span},
  widgets::{Paragraph, Widget},
};

use crate::{
  components::{Render, input::Input},
  signals::{Signal, signal},
  style::{Style, palette},
};

/// The painted parts of an [`Autocomplete`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AutocompleteStyle {
  /// The marker on the suggestion the arrows are on.
  pub active_symbol: &'static str,
  /// The marker on every other suggestion, same width so the list doesn't jitter.
  pub inactive_symbol: &'static str,
  /// The suggestion the arrows are on.
  pub highlighted: Style,
  /// Every other suggestion.
  pub inactive: Style,
  /// The aside on the highlighted suggestion.
  pub note: Style,
}

impl Default for AutocompleteStyle {
  fn default() -> Self {
    Self {
      active_symbol: "❯",
      inactive_symbol: "·",
      highlighted: Style::new().fg(palette::BASE).bg(palette::MAUVE),
      inactive: Style::new().fg(palette::OVERLAY1),
      note: Style::new().fg(palette::OVERLAY0).italic(),
    }
  }
}

/// A text field with a list of suggestions underneath, narrowed to what fuzzy-matches as you
/// type.
///
/// Typing edits the field — `Autocomplete` composes [`Input`], so word movement, whole-word
/// deletion and selection all come along for free. Arrows move a highlight through the
/// suggestions instead of the caret; Enter accepts whichever is highlighted (copying it into
/// the field) and lets the prompt submit, or — with nothing highlighted — submits whatever
/// was typed, list or no list.
///
/// Strictness is not this type's business: whether a typed value that matches nothing counts
/// as an answer is a question for the [`Ask`](crate::components::Ask) refusal a caller wires
/// up, same as any other field.
#[derive(Clone, Copy)]
pub struct Autocomplete {
  input: Input,
  options: Signal<Vec<Cow<'static, str>>>,
  /// One per option, empty where there is none. Only the highlighted suggestion shows its
  /// own.
  notes: Signal<Vec<Cow<'static, str>>>,
  /// A position into the current matches, not into `options` — the same row of *options*
  /// can sit at a different position once the query changes.
  highlighted: Signal<Option<usize>>,
  style: Signal<AutocompleteStyle>,
}

impl Autocomplete {
  pub fn new(options: impl IntoIterator<Item: Into<Cow<'static, str>>>) -> Self {
    Self {
      input: Input::new(),
      options: signal(options.into_iter().map(Into::into).collect()),
      notes: signal(Vec::new()),
      highlighted: signal(None),
      style: signal(AutocompleteStyle::default()),
    }
  }

  /// Muted hint text, shown only while the field is empty.
  #[must_use]
  pub fn placeholder(self, text: &'static str) -> Self {
    Self {
      input: self.input.placeholder(text),
      ..self
    }
  }

  /// An aside for one option, shown only while it's the highlighted suggestion.
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
  pub fn style(self, style: AutocompleteStyle) -> Self {
    self.style.set(style);
    self
  }

  /// Whatever's currently typed — a suggestion the user accepted, or free text.
  #[must_use]
  pub fn value(&self) -> String {
    self.input.value()
  }

  /// The absolute indices into `options` that fuzzy-match the field's own text, checked
  /// against an option's label or its note (a hint is content too — same rule `Select` and
  /// `MultiSelect` search use). Recomputed on every keystroke and every frame — cheap for the
  /// sizes a prompt list actually reaches.
  fn matches(&self) -> Vec<usize> {
    let query = self.input.value();
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

  /// Moves the highlight by `by` positions among the current matches, wrapping. Starts at the
  /// near end rather than the middle of the list: Down from nothing lands on the first
  /// suggestion, Up from nothing lands on the last.
  fn move_highlight(&self, by: i32) {
    let matches = self.matches();
    if matches.is_empty() {
      self.highlighted.set(None);
      return;
    }

    let next = match self.highlighted.get() {
      Some(at) => (at as i32 + by).rem_euclid(matches.len() as i32) as usize,
      None if by > 0 => 0,
      None => matches.len() - 1,
    };
    self.highlighted.set(Some(next));
  }

  /// The option text the highlight is on, if any.
  fn highlighted_option(&self) -> Option<Cow<'static, str>> {
    let matches = self.matches();
    let at = self.highlighted.get()?;
    let index = *matches.get(at)?;
    self.options.with_ref(|opts| opts.get(index).cloned())
  }

  pub fn on_key(&self, key: KeyEvent) -> bool {
    match key.code {
      KeyCode::Up => {
        self.move_highlight(-1);
        true
      }
      KeyCode::Down => {
        self.move_highlight(1);
        true
      }
      // Not consumed: a suggestion, once copied in, is what `ask` reads Enter as submitting.
      // Nothing highlighted just leaves the typed text as the answer.
      KeyCode::Enter => {
        if let Some(option) = self.highlighted_option() {
          self.input.set(option);
        }
        false
      }
      _ => {
        let consumed = self.input.on_key(key);
        if consumed {
          // The list just changed under it, or the caret moved off it entirely — either way
          // the old highlight no longer means anything.
          self.highlighted.set(None);
        }
        consumed
      }
    }
  }
}

impl crate::components::Ask for Autocomplete {
  fn answer(&self) -> String {
    self.value()
  }

  fn controls(&self) -> &'static str {
    "type to filter · ↑↓ to pick a suggestion · enter to submit"
  }
}

impl Render for Autocomplete {
  fn height(&self, width: u16) -> u16 {
    self.input.height(width) + self.matches().len() as u16
  }

  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let matches = self.matches();
    let style = self.style.get();
    let highlighted = self.highlighted.get();
    let notes = self.notes.get();

    let input_height = self.input.height(area.width()).min(area.height());
    let input_area = ratatui::layout::Rect {
      x: area.x(),
      y: area.y(),
      width: area.width(),
      height: input_height,
    };
    self.input.render(input_area.into(), buf);

    if area.height() <= input_height {
      return;
    }

    let lines: Vec<Line> = self.options.with_ref(|opts| {
      matches
        .iter()
        .enumerate()
        .map(|(pos, &at)| {
          let option = &opts[at];
          let on = Some(pos) == highlighted;
          let row_style = if on { style.highlighted } else { style.inactive };
          let symbol = if on { style.active_symbol } else { style.inactive_symbol };

          let mut spans = vec![
            Span::styled(symbol, row_style),
            Span::styled(format!(" {option}"), row_style),
          ];
          if on && let Some(note) = notes.get(at).filter(|note| !note.is_empty()) {
            spans.push(Span::styled(format!(" {note}"), style.note));
          }
          Line::from(spans)
        })
        .collect()
    });

    let list_area = ratatui::layout::Rect {
      x: area.x(),
      y: area.y() + input_height,
      width: area.width(),
      height: area.height() - input_height,
    };
    Widget::render(Paragraph::new(lines), list_area, buf.inner_mut());
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

  fn wide(field: Autocomplete, rows: u16, width: u16) -> Vec<String> {
    let mut terminal = ratatui::Terminal::new(TestBackend::new(width, rows)).unwrap();
    terminal
      .draw(|frame| {
        let area = frame.area().into();
        let mut buf = Buffer::from(frame.buffer_mut());
        field.render(area, &mut buf);
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

  #[test]
  fn typing_narrows_the_suggestions() {
    let field = Autocomplete::new(["bun", "deno", "node"]);
    for c in "de".chars() {
      assert!(field.on_key(press(KeyCode::Char(c))));
    }

    assert_eq!(wide(field, 2, 20), ["de", "· deno"]);
  }

  #[test]
  fn down_highlights_the_first_suggestion_and_enter_accepts_it() {
    let field = Autocomplete::new(["bun", "deno", "node"]);
    for c in "e".chars() {
      field.on_key(press(KeyCode::Char(c)));
    }

    assert!(field.on_key(press(KeyCode::Down)));
    assert_eq!(
      wide(field, 3, 20),
      ["e", "❯ deno", "· node"],
      "deno is highlighted first"
    );

    // Not consumed, so `ask` reads it as submit — but the field's own value has to change
    // first, or the answer would just be "e".
    assert!(!field.on_key(press(KeyCode::Enter)));
    assert_eq!(field.value(), "deno");
  }

  #[test]
  fn enter_with_nothing_highlighted_leaves_the_typed_text_alone() {
    let field = Autocomplete::new(["bun", "deno"]);
    for c in "xyz".chars() {
      field.on_key(press(KeyCode::Char(c)));
    }

    assert!(!field.on_key(press(KeyCode::Enter)));
    assert_eq!(field.value(), "xyz", "no matches, no substitution");
  }

  #[test]
  fn down_wraps_and_up_from_nothing_starts_at_the_end() {
    let field = Autocomplete::new(["bun", "deno", "node"]);
    for _ in 0..4 {
      field.on_key(press(KeyCode::Down));
    }
    assert_eq!(field.highlighted_option(), Some(Cow::Borrowed("bun")), "wrapped back to the top");

    let fresh = Autocomplete::new(["bun", "deno", "node"]);
    fresh.on_key(press(KeyCode::Up));
    assert_eq!(fresh.highlighted_option(), Some(Cow::Borrowed("node")));
  }

  #[test]
  fn editing_the_text_drops_a_stale_highlight() {
    let field = Autocomplete::new(["bun", "deno", "node"]);
    field.on_key(press(KeyCode::Down));
    assert!(field.highlighted_option().is_some());

    field.on_key(press(KeyCode::Char('x')));
    assert_eq!(field.highlighted_option(), None);
  }

  #[test]
  fn a_query_can_match_the_note_instead_of_the_label() {
    let field = Autocomplete::new(["bun", "pnpm", "npm"]).note(0, "(recommended)");
    for c in "recom".chars() {
      field.on_key(press(KeyCode::Char(c)));
    }

    assert_eq!(field.matches(), vec![0]);
  }
}
