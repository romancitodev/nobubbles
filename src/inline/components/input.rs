use eyre::Result;

use crate::{components::input::Input, inline::components::ask};

/// Asks for a line of text.
///
/// # Errors
/// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
pub fn input(prompt: &str) -> Result<String> {
  let field = Input::new();
  ask(prompt, field, |key| field.on_key(key))?;

  Ok(field.value())
}

/// Asks for text across several lines. Enter breaks the line, `ctrl+s` submits.
///
/// # Errors
/// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
pub fn multiline(prompt: &str) -> Result<String> {
  let field = Input::new().multiline();
  ask(prompt, field, |key| field.on_key(key))?;

  Ok(field.value())
}
