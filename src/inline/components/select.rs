use std::borrow::Cow;
use std::iter::once;

use eyre::Result;

use crate::{
  components::select::Select as Widget,
  inline::components::{always_ok, ask},
};

/// Asks the user to pick one option.
///
/// Optional by default: `ask` gives back `None` when the user takes the skip row, which the
/// prompt draws and your list never contains. Call [`Select::strict`] when there is no such
/// thing as no answer.
///
/// ```no_run
/// # use nobubbles::inline;
/// let scope = inline::select("Scope").items(["api", "ui"]).ask()?;           // Option<usize>
/// let kind = inline::select("Type").items(["feat", "fix"]).strict().ask()?;  // usize
/// # Ok::<(), eyre::Report>(())
/// ```
pub struct Select<'a> {
  prompt: &'a str,
  options: Vec<Cow<'static, str>>,
  initial: usize,
  max_rows: Option<u16>,
  skip: &'static str,
}

/// Opens a single-choice prompt.
pub fn select(prompt: &str) -> Select<'_> {
  Select {
    prompt,
    options: Vec::new(),
    initial: 0,
    max_rows: None,
    skip: "none",
  }
}

impl<'a> Select<'a> {
  /// The options to choose from. Indices into this are what `ask` gives back.
  #[must_use]
  pub fn items(self, options: impl IntoIterator<Item: Into<Cow<'static, str>>>) -> Self {
    Self {
      options: options.into_iter().map(Into::into).collect(),
      ..self
    }
  }

  /// Starts the cursor on this option instead of the first.
  #[must_use]
  pub fn initial(self, index: usize) -> Self {
    Self {
      initial: index,
      ..self
    }
  }

  /// Shows at most `rows` options at a time, scrolling to keep the cursor in view.
  #[must_use]
  pub fn max_rows(self, rows: u16) -> Self {
    Self {
      max_rows: Some(rows),
      ..self
    }
  }

  /// Renames the skip row. Ignored under [`Select::strict`].
  #[must_use]
  pub fn skip(self, label: &'static str) -> Self {
    Self {
      skip: label,
      ..self
    }
  }

  /// Requires an answer: no skip row, and `ask` gives back a plain index.
  ///
  /// Call it last, after the rest of the configuration.
  #[must_use]
  pub fn strict(self) -> Strict<'a> {
    Strict(self)
  }

  /// Runs it, giving back the index picked or `None` for the skip row.
  ///
  /// # Errors
  /// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
  pub fn ask(self) -> Result<Option<usize>> {
    // The skip row is the prompt's, never the caller's list. That is the whole point: no
    // sentinel to insert, no string to compare back out, and the type says it can be absent.
    let items = once(Cow::Borrowed(self.skip)).chain(self.options.iter().cloned());
    let picked = run(items, self.initial + 1, self.max_rows, self.prompt)?;

    Ok(picked.checked_sub(1))
  }
}

/// A [`Select`] that will not take no for an answer.
pub struct Strict<'a>(Select<'a>);

impl Strict<'_> {
  /// Runs it, giving back the index picked.
  ///
  /// # Errors
  /// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
  pub fn ask(self) -> Result<usize> {
    let Select {
      prompt,
      options,
      initial,
      max_rows,
      ..
    } = self.0;

    run(options.into_iter(), initial, max_rows, prompt)
  }
}

fn run(
  items: impl Iterator<Item = Cow<'static, str>>,
  initial: usize,
  max_rows: Option<u16>,
  prompt: &str,
) -> Result<usize> {
  let mut list = Widget::new(items);
  if let Some(rows) = max_rows {
    list = list.max_rows(rows);
  }
  let list = list.initial(initial);

  ask(prompt, list, |key| list.on_key(key), &always_ok)?;
  Ok(list.selected())
}
