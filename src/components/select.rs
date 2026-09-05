use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
  text::Line,
  widgets::{Paragraph, Widget},
};

use crate::{
  components::Render,
  signals::{Signal, signal},
};

#[derive(Clone, Copy)]
pub struct Select {
  options: Signal<Vec<Cow<'static, str>>>,
  selected: Signal<usize>,
}

impl Select {
  pub fn new(options: impl Iterator<Item: Into<Cow<'static, str>>>) -> Self {
    let options: Vec<Cow<'_, str>> = options.map(Into::into).collect();
    Self {
      options: signal(options),
      selected: signal(0),
    }
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
    let lines: Vec<Line> = self.options.with_ref(|opts| {
      opts
        .iter()
        .enumerate()
        .map(|(i, o)| {
          if i == selected {
            Line::from(format!("> {o}"))
          } else {
            Line::from(format!("  {o}"))
          }
        })
        .collect()
    });
    Widget::render(Paragraph::new(lines), area.into(), buf.inner_mut());
  }
}
