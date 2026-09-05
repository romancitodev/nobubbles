use eyre::Result;

use crate::{
  components::input::Input,
  inline::components::{Check, Validator, always_ok, ask},
};

/// Asks for something secret. Dots on screen, dots in the transcript.
pub struct Password<'a> {
  prompt: &'a str,
  check: Option<Validator<'a>>,
}

/// Opens a masked text prompt.
pub fn password(prompt: &str) -> Password<'_> {
  Password {
    prompt,
    check: None,
  }
}

impl<'a> Password<'a> {
  /// Refuses the answer with a reason instead of submitting it. Sees the real value, which
  /// is the whole reason a length rule can work here.
  #[must_use]
  pub fn validate(self, check: impl Fn(&str) -> Result<(), String> + 'a) -> Self {
    Self {
      check: Some(Box::new(check)),
      ..self
    }
  }

  /// Runs it.
  ///
  /// # Errors
  /// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
  pub fn ask(self) -> Result<String> {
    let field = Input::new().masked();

    // The check runs on the real value, not on the dots `answer()` reports.
    let check = self.check;
    let real = move |_: &str| match check.as_deref() {
      Some(check) => check(&field.value()),
      None => Ok(()),
    };
    let check: Check<'_> = &real;

    ask(self.prompt, field, |key| field.on_key(key), check)?;
    let _ = always_ok;

    Ok(field.value())
  }
}
