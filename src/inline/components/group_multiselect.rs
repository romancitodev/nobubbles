use std::borrow::Cow;

use eyre::Result;

use crate::{
  rimel::Block,
  components::multiselect::MultiSelect as Widget,
  inline::components::{Check, Validator, always_ok, ask},
};

/// Asks the user to tick options that come in groups.
///
/// ```no_run
/// # use nobubbles::inline;
/// let picked = inline::group_multiselect("Dependencies")
///   .group("Frontend", ["react", "svelte"])
///   .group("Backend", ["axum", "actix"])
///   .ask()?;
/// # Ok::<(), eyre::Report>(())
/// ```
///
/// Headings are drawn without a box and the cursor jumps over them. The indices that come
/// back count only the options, in the order they were added, so they line up with the same
/// list flattened by hand.
pub struct GroupMultiSelect<'a> {
  prompt: Block,
  rows: Vec<Cow<'static, str>>,
  headings: Vec<usize>,
  max_rows: Option<u16>,
  check: Option<Validator<'a>>,
}

/// Opens a grouped multiple-choice prompt.
pub fn group_multiselect(prompt: impl Into<Block>) -> GroupMultiSelect<'static> {
  GroupMultiSelect {
    prompt: prompt.into(),
    rows: Vec::new(),
    headings: Vec::new(),
    max_rows: None,
    check: None,
  }
}

impl<'a> GroupMultiSelect<'a> {
  /// A heading and the options under it.
  #[must_use]
  pub fn group(
    mut self,
    title: impl Into<Cow<'static, str>>,
    options: impl IntoIterator<Item: Into<Cow<'static, str>>>,
  ) -> Self {
    self.headings.push(self.rows.len());
    self.rows.push(title.into());
    self.rows.extend(options.into_iter().map(Into::into));
    self
  }

  /// Shows at most `rows` rows at a time, headings included, scrolling to follow the cursor.
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

  /// Runs it, giving back the option indices ticked, headings not counted.
  ///
  /// # Errors
  /// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
  pub fn ask(self) -> Result<Vec<usize>> {
    let mut list = Widget::new(self.rows);
    if let Some(rows) = self.max_rows {
      list = list.max_rows(rows);
    }
    let list = list.headers(self.headings);

    let check: Check<'_> = self.check.as_deref().unwrap_or(&always_ok);
    ask(self.prompt, list, |key| list.on_key(key), check)?;

    // The widget answers in row numbers, which count the headings. The caller never saw
    // those, so they get mapped back out here.
    let options = list.rows();
    Ok(
      list
        .selected()
        .into_iter()
        .filter_map(|row| options.iter().position(|&option| option == row))
        .collect(),
    )
  }
}
