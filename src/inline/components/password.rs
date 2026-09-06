use eyre::Result;

use crate::{
  rimel::Block,
  components::input::Input,
  inline::components::{Check, Validator, always_ok, ask},
};

/// Asks for something secret. Dots on screen, dots in the transcript.
pub struct Password<'a> {
  prompt: Block,
  placeholder: Option<&'static str>,
  check: Option<Validator<'a>>,
  invisible: bool,
}

/// Opens a masked text prompt.
pub fn password(prompt: impl Into<Block>) -> Password<'static> {
  Password {
    prompt: prompt.into(),
    placeholder: None,
    check: None,
    invisible: false,
  }
}

impl<'a> Password<'a> {
  /// Muted hint text, shown only while the field is empty.
  #[must_use]
  pub fn placeholder(self, text: &'static str) -> Self {
    Self {
      placeholder: Some(text),
      ..self
    }
  }

  /// Past the usual dots: draws nothing at all while typing, for the rare case where even
  /// "someone's typing something" shouldn't show.
  #[must_use]
  pub fn invisible(self) -> Self {
    Self {
      invisible: true,
      ..self
    }
  }

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
    let mut field = if self.invisible {
      Input::new().invisible()
    } else {
      Input::new().masked()
    };
    if let Some(placeholder) = self.placeholder {
      field = field.placeholder(placeholder);
    }

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
