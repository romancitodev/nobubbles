use std::borrow::Cow;

use eyre::Result;

use crate::{components::select::Select, inline::components::ask};

/// Asks the user to pick one option, and returns its **index**.
///
/// The index and not the text: the caller usually has the real thing behind it, an id, an
/// enum variant, a row, and matching on a string it just formatted is the long way around.
///
/// # Errors
/// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
pub fn select(
  prompt: &str,
  options: impl IntoIterator<Item: Into<Cow<'static, str>>>,
) -> Result<usize> {
  let list = Select::new(options.into_iter());
  ask(prompt, list, |key| {
    let _ = list.on_key(key);
  })?;

  Ok(list.selected())
}
