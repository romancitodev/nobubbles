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

/// The painted parts of a [`Select`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SelectStyle {
  pub active_symbol: &'static str,
  pub inactive_symbol: &'static str,
  /// The picked row's marker. Its label stays plain, so the eye lands on the marker.
  pub active: Style,
  /// Every other row, marker and label together.
  pub inactive: Style,
  /// Most rows drawn at once. `None` draws the whole list, however long it is.
  pub max_rows: Option<u16>,
}

impl Default for SelectStyle {
  fn default() -> Self {
    Self {
      active_symbol: "●",
      inactive_symbol: "○",
      active: Style::new().fg(Color::Green),
      inactive: Style::new().dim(),
      max_rows: None,
    }
  }
}

#[derive(Clone, Copy)]
pub struct Select {
  options: Signal<Vec<Cow<'static, str>>>,
  selected: Signal<usize>,
  /// First row of the window into `options`. Only moves when the cursor would leave it.
  offset: Signal<usize>,
  style: Signal<SelectStyle>,
}

impl Select {
  pub fn new(options: impl Iterator<Item: Into<Cow<'static, str>>>) -> Self {
    let options: Vec<Cow<'_, str>> = options.map(Into::into).collect();
    Self {
      options: signal(options),
      selected: signal(0),
      offset: signal(0),
      style: signal(SelectStyle::default()),
    }
  }

  /// Starts the cursor on `index` instead of the first row. Out of range lands on the first.
  #[must_use]
  pub fn initial(self, index: usize) -> Self {
    let len = self.options.with_ref(Vec::len);
    if index < len {
      self.selected.set(index);
      self.scroll_into_view();
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

  /// Replaces the style.
  #[must_use]
  pub fn style(self, style: SelectStyle) -> Self {
    self.style.set(style);
    self
  }

  pub fn on_key(&self, key: KeyEvent) -> bool {
    match key.code {
      KeyCode::Up => {
        let len = self.options.with_ref(Vec::len);
        if len > 0 {
          self.selected.update(|s| *s = (*s + len - 1) % len);
          self.scroll_into_view();
        }
        true
      }
      KeyCode::Down => {
        let len = self.options.with_ref(Vec::len);
        if len > 0 {
          self.selected.update(|s| *s = (*s + 1) % len);
          self.scroll_into_view();
        }
        true
      }
      _ => false,
    }
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
    let cursor = self.selected.get();
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
    "↑↓ to move · enter to submit"
  }
}

impl Render for Select {
  fn height(&self, _: u16) -> u16 {
    self.window().len() as u16
  }

  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let selected = self.selected.get();
    let style = self.style.get();
    let window = self.window();

    let lines: Vec<Line> = self.options.with_ref(|opts| {
      opts
        .iter()
        .enumerate()
        .skip(window.start)
        .take(window.len())
        .map(|(i, option)| {
          if i == selected {
            Line::from(vec![
              Span::styled(style.active_symbol, style.active),
              Span::raw(format!(" {option}")),
            ])
          } else {
            Line::styled(
              format!("{} {option}", style.inactive_symbol),
              style.inactive,
            )
          }
        })
        .collect()
    });
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
    let mut terminal = ratatui::Terminal::new(TestBackend::new(12, rows)).unwrap();
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
        (0..12)
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

  #[test]
  fn the_window_only_moves_once_the_cursor_would_leave_it() {
    let list = digits(10);
    assert_eq!(visible(list, 3), ["● 0", "○ 1", "○ 2"]);

    list.on_key(press(KeyCode::Down));
    list.on_key(press(KeyCode::Down));
    assert_eq!(
      visible(list, 3),
      ["○ 0", "○ 1", "● 2"],
      "still in view, no scroll"
    );

    list.on_key(press(KeyCode::Down));
    assert_eq!(
      visible(list, 3),
      ["○ 1", "○ 2", "● 3"],
      "now it scrolls, by one"
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
}
