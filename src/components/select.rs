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
}

impl Default for SelectStyle {
  fn default() -> Self {
    Self {
      active_symbol: "●",
      inactive_symbol: "○",
      active: Style::new().fg(Color::Green),
      inactive: Style::new().dim(),
    }
  }
}

#[derive(Clone, Copy)]
pub struct Select {
  options: Signal<Vec<Cow<'static, str>>>,
  selected: Signal<usize>,
  style: Signal<SelectStyle>,
}

impl Select {
  pub fn new(options: impl Iterator<Item: Into<Cow<'static, str>>>) -> Self {
    let options: Vec<Cow<'_, str>> = options.map(Into::into).collect();
    Self {
      options: signal(options),
      selected: signal(0),
      style: signal(SelectStyle::default()),
    }
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
        }
        true
      }
      KeyCode::Down => {
        let len = self.options.with_ref(Vec::len);
        if len > 0 {
          self.selected.update(|s| *s = (*s + 1) % len);
        }
        true
      }
      _ => false,
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

impl Render for Select {
  fn height(&self, _: u16) -> u16 {
    self.options.with_ref(|options| options.len()) as u16
  }

  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let selected = self.selected.get();
    let style = self.style.get();

    let lines: Vec<Line> = self.options.with_ref(|opts| {
      opts
        .iter()
        .enumerate()
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
