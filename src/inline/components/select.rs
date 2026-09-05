use std::borrow::Cow;

use crossterm::event::KeyCode;
use eyre::Result;
use ratatui::widgets::Paragraph;

use crate::{app::Inline, column, components::select::Select, signals};

/// Displays a prompt with a list of options and waits for the user to pick one with the
/// arrow keys. Returns the **index** of the pick, not the text: the caller usually has the
/// real thing behind that index — an id, an enum variant, a row — and matching on a string
/// it just formatted would be the long way around.
///
/// # Errors
/// Can return an error if the terminal cannot be initialized or if there is an issue with rendering.
pub fn select(
  prompt: &str,
  options: impl IntoIterator<Item: Into<Cow<'static, str>>>,
) -> Result<usize> {
  let list = Select::new(options.into_iter());
  // `Paragraph` borrows what it renders and `Column` needs its children `'static`, so the
  // label is owned. One small allocation per repaint, for one line of text.
  let label = prompt.to_owned();

  Inline::run(signals::fps(), |cx| {
    if let Some(key) = cx.key() {
      if key.code == KeyCode::Enter {
        signals::quit();
      } else {
        let _ = list.on_key(key);
      }
    }

    cx.render(column![Paragraph::new(label.clone()), list]);
  })?;

  Ok(list.selected())
}
