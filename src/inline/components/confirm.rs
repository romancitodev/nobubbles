use eyre::Result;

use crate::{
  components::confirm::Confirm as Widget,
  inline::components::{always_ok, ask},
};

/// Asks a yes or no. Arrows flip the answer, `y` and `n` say it outright.
pub struct Confirm<'a> {
  prompt: &'a str,
  choice: Widget,
}

/// Opens a yes/no prompt, starting on yes.
pub fn confirm(prompt: &str) -> Confirm<'_> {
  Confirm {
    prompt,
    choice: Widget::new(),
  }
}

impl Confirm<'_> {
  /// Starts on `value` instead of yes.
  #[must_use]
  pub fn initial(self, value: bool) -> Self {
    Self {
      choice: self.choice.initial(value),
      ..self
    }
  }

  /// Runs it.
  ///
  /// # Errors
  /// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
  pub fn ask(self) -> Result<bool> {
    let choice = self.choice;
    ask(self.prompt, choice, |key| choice.on_key(key), &always_ok)?;
    Ok(choice.value())
  }
}
