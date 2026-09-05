use eyre::Result;

use crate::{
  components::input::Input,
  inline::components::{Check, Validator, always_ok, ask},
};

/// Asks for text. Configure it, then [`Text::ask`].
///
/// ```no_run
/// # use nobubbles::inline;
/// let title = inline::input("Title")
///   .validate(|t| if t.is_empty() { Err("required".into()) } else { Ok(()) })
///   .ask()?;
/// # Ok::<(), eyre::Report>(())
/// ```
pub struct Text<'a> {
  prompt: &'a str,
  field: Input,
  check: Option<Validator<'a>>,
}

/// Opens a text prompt.
pub fn input(prompt: &str) -> Text<'_> {
  Text {
    prompt,
    field: Input::new(),
    check: None,
  }
}

impl<'a> Text<'a> {
  /// Pre-fills the field.
  #[must_use]
  pub fn initial(self, value: impl Into<String>) -> Self {
    Self {
      field: Input::with(value),
      ..self
    }
  }

  /// Enter breaks the line instead of submitting; `ctrl+s` submits.
  #[must_use]
  pub fn multiline(self) -> Self {
    Self {
      field: self.field.multiline(),
      ..self
    }
  }

  /// Refuses the answer with a reason instead of submitting it.
  ///
  /// The prompt stays live and turns yellow with the message on the closer, so a refusal is
  /// not a cancel.
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
    let field = self.field;
    let check: Check<'_> = self.check.as_deref().unwrap_or(&always_ok);

    ask(self.prompt, field, |key| field.on_key(key), check)?;
    Ok(field.value())
  }
}
