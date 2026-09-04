use eyre::Result;
use nobubbles::app::Inline;
use nobubbles::components::{Component, Render};
use nobubbles::signals::{Signal, quit, signal};
use ratatui::widgets::Paragraph;

struct Counter {
  count: Signal<i32>,
}

impl Component for Counter {
  fn view(&self) -> impl Render {
    Paragraph::new(format!("count: {} (up/down, q to quit)", self.count))
  }
}

fn main() -> Result<()> {
  let counter = Counter { count: signal(0) };

  Inline::run(30, |cx| {
    if let Some(key) = cx.key() {
      match key.code {
        crossterm::event::KeyCode::Up => counter.count.update(|c| *c += 1),
        crossterm::event::KeyCode::Down => counter.count.update(|c| *c -= 1),
        crossterm::event::KeyCode::Char('q') => quit(),
        _ => {}
      }
    }

    cx.render(&counter);
  })
}
