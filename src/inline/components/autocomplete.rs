use std::borrow::Cow;

use eyre::Result;

use crate::{
  rimel::Block,
  components::autocomplete::Autocomplete as Widget,
  inline::components::{Check, Validator, always_ok, ask},
};

/// Asks for text with a list of suggestions underneath, narrowed to what fuzzy-matches as you
/// type.
///
/// Relaxed by default: typed text that matches nothing is still a valid answer, for the
/// "pick a quick word, or make up a new one" case — a tag field, say. Call
/// [`Autocomplete::strict`] when the answer has to be one of the options, same idea as
/// [`Select::strict`](crate::inline::Select::strict).
///
/// ```no_run
/// # use nobubbles::inline;
/// let tag = inline::autocomplete("Tag").items(["bug", "feature", "chore"]).ask()?;
/// let exact = inline::autocomplete("Package").items(["bun", "pnpm", "npm"]).strict().ask()?;
/// # Ok::<(), eyre::Report>(())
/// ```
pub struct Autocomplete<'a> {
  prompt: Block,
  options: Vec<Cow<'static, str>>,
  notes: Vec<(usize, Cow<'static, str>)>,
  placeholder: Option<&'static str>,
  strict: bool,
  check: Option<Validator<'a>>,
}

/// Opens an autocomplete prompt.
pub fn autocomplete(prompt: impl Into<Block>) -> Autocomplete<'static> {
  Autocomplete {
    prompt: prompt.into(),
    options: Vec::new(),
    notes: Vec::new(),
    placeholder: None,
    strict: false,
    check: None,
  }
}

impl<'a> Autocomplete<'a> {
  /// The suggestions offered as you type.
  #[must_use]
  pub fn items(self, options: impl IntoIterator<Item: Into<Cow<'static, str>>>) -> Self {
    Self {
      options: options.into_iter().map(Into::into).collect(),
      ..self
    }
  }

  /// An aside for one option, shown only while it's the highlighted suggestion.
  #[must_use]
  pub fn note(mut self, at: usize, text: impl Into<Cow<'static, str>>) -> Self {
    self.notes.push((at, text.into()));
    self
  }

  /// Muted hint text, shown only while the field is empty.
  #[must_use]
  pub fn placeholder(self, text: &'static str) -> Self {
    Self {
      placeholder: Some(text),
      ..self
    }
  }

  /// Requires the answer to be one of [`Autocomplete::items`], word for word. A typed value
  /// that matches nothing is refused the same way a custom [`Autocomplete::validate`] would
  /// refuse it — the closer turns yellow and says why, rather than the prompt ending.
  ///
  /// Ignored if [`Autocomplete::validate`] is also called: a custom check replaces this one
  /// rather than combining with it.
  #[must_use]
  pub fn strict(self) -> Self {
    Self {
      strict: true,
      ..self
    }
  }

  /// Refuses the answer with a reason instead of submitting it. Takes over from
  /// [`Autocomplete::strict`] if both are called.
  #[must_use]
  pub fn validate(self, check: impl Fn(&str) -> Result<(), String> + 'a) -> Self {
    Self {
      check: Some(Box::new(check)),
      ..self
    }
  }

  /// Runs it, giving back whatever was typed or accepted.
  ///
  /// # Errors
  /// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
  pub fn ask(self) -> Result<String> {
    let mut list = Widget::new(self.options.iter().cloned());
    for (at, note) in self.notes {
      list = list.note(at, note);
    }
    if let Some(placeholder) = self.placeholder {
      list = list.placeholder(placeholder);
    }

    let options = self.options;
    let one_of_the_options = move |text: &str| {
      if options.iter().any(|option| option == text) {
        Ok(())
      } else {
        Err("pick one from the list".to_owned())
      }
    };

    let check: Check<'_> = match self.check.as_deref() {
      Some(custom) => custom,
      None if self.strict => &one_of_the_options,
      None => &always_ok,
    };

    ask(self.prompt, list, |key| list.on_key(key), check)?;
    Ok(list.value())
  }
}
