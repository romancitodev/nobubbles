use std::borrow::Cow;

use eyre::Result;

use crate::{
  rimel::Block,
  components::multiselect::MultiSelect as Widget,
  inline::components::{Check, Validator, always_ok, ask},
};

/// Asks the user to tick any number of options.
///
/// Arrows move, space ticks, Enter submits. An empty pick is a valid answer unless a
/// [`MultiSelect::validate`] says otherwise.
pub struct MultiSelect<'a> {
  prompt: Block,
  options: Vec<Cow<'static, str>>,
  /// Asides, by option index.
  notes: Vec<(usize, Cow<'static, str>)>,
  max_rows: Option<u16>,
  check: Option<Validator<'a>>,
}

/// Opens a multiple-choice prompt.
pub fn multiselect(prompt: impl Into<Block>) -> MultiSelect<'static> {
  MultiSelect {
    prompt: prompt.into(),
    options: Vec::new(),
    notes: Vec::new(),
    max_rows: None,
    check: None,
  }
}

impl<'a> MultiSelect<'a> {
  /// The options to choose from. Indices into this are what `ask` gives back.
  #[must_use]
  pub fn items(self, options: impl IntoIterator<Item: Into<Cow<'static, str>>>) -> Self {
    Self {
      options: options.into_iter().map(Into::into).collect(),
      ..self
    }
  }

  /// An aside for one option, shown only while the cursor is on it.
  #[must_use]
  pub fn note(mut self, at: usize, text: impl Into<Cow<'static, str>>) -> Self {
    self.notes.push((at, text.into()));
    self
  }

  /// Shows at most `rows` options at a time, scrolling to keep the cursor in view.
  #[must_use]
  pub fn max_rows(self, rows: u16) -> Self {
    Self {
      max_rows: Some(rows),
      ..self
    }
  }

  /// Refuses the pick with a reason instead of submitting it. Sees the ticked labels joined
  /// by commas, or `none`.
  #[must_use]
  pub fn validate(self, check: impl Fn(&str) -> Result<(), String> + 'a) -> Self {
    Self {
      check: Some(Box::new(check)),
      ..self
    }
  }

  /// Runs it, giving back the ticked indices in order.
  ///
  /// # Errors
  /// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
  pub fn ask(self) -> Result<Vec<usize>> {
    let mut list = Widget::new(self.options);
    if let Some(rows) = self.max_rows {
      list = list.max_rows(rows);
    }
    for (at, note) in self.notes {
      list = list.note(at, note);
    }

    let check: Check<'_> = self.check.as_deref().unwrap_or(&always_ok);
    ask(self.prompt, list, |key| list.on_key(key), check)?;

    Ok(list.selected())
  }
}
