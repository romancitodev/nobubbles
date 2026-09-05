use eyre::Result;

use crate::{components::confirm::Confirm, inline::components::ask};

/// Asks a yes or no. Arrows flip the answer, `y` and `n` say it outright.
///
/// # Errors
/// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
pub fn confirm(prompt: &str) -> Result<bool> {
  let choice = Confirm::new();
  ask(prompt, choice, |key| choice.on_key(key))?;

  Ok(choice.value())
}
