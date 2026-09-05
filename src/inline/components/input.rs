use crossterm::event::KeyCode;
use eyre::Result;
use ratatui::widgets::Paragraph;

use crate::{app::Inline, components::input::Input, signals};

/// Displays a prompt and waits for user input. Returns the input as a `String`.
///
/// # Errors
/// Can return an error if the terminal cannot be initialized or if there is an issue with rendering.
pub fn input(prompt: &str) -> Result<String> {
  let field = Input::new();
  Inline::run(signals::fps(), |cx| {
    if let Some(key) = cx.key() {
      if key.code == KeyCode::Enter {
        signals::quit();
      } else {
        let _ = field.on_key(key);
      }
    }

    cx.render(Paragraph::new(format!("{}{}", prompt, field.value())));
  })?;

  Ok(field.value())
}
