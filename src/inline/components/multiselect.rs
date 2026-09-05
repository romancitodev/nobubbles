use std::borrow::Cow;

use eyre::Result;

use crate::{components::multiselect::MultiSelect, inline::components::ask};

/// Asks the user to tick any number of options, and returns their **indices**, in order.
///
/// Arrows move, space ticks, Enter submits. An empty pick is a valid answer.
///
/// # Errors
/// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
pub fn multiselect(
  prompt: &str,
  options: impl IntoIterator<Item: Into<Cow<'static, str>>>,
) -> Result<Vec<usize>> {
  let list = MultiSelect::new(options);
  ask(prompt, list, |key| list.on_key(key))?;

  Ok(list.selected())
}
