use std::borrow::Cow;

use eyre::Result;

use crate::{
  rimel::Block,
  components::select_key::SelectKey as Widget,
  inline::components::{always_ok, ask},
};

/// Asks a question you answer by pressing a letter.
///
/// ```no_run
/// # use nobubbles::inline;
/// let choice = inline::select_key("Commit this?")
///   .items([('y', "yes, ship it"), ('n', "no, hold on"), ('e', "edit the message")])
///   .ask()?;
/// # Ok::<(), eyre::Report>(())
/// ```
pub struct SelectKey {
  prompt: Block,
  options: Vec<(char, Cow<'static, str>)>,
}

/// Opens a key-driven prompt.
pub fn select_key(prompt: impl Into<Block>) -> SelectKey {
  SelectKey {
    prompt: prompt.into(),
    options: Vec::new(),
  }
}

impl SelectKey {
  /// The keys and what they mean. Indices into this are what `ask` gives back.
  #[must_use]
  pub fn items(
    self,
    options: impl IntoIterator<Item = (char, impl Into<Cow<'static, str>>)>,
  ) -> Self {
    Self {
      options: options
        .into_iter()
        .map(|(key, label)| (key, label.into()))
        .collect(),
      ..self
    }
  }

  /// Runs it, giving back the index of the key that was pressed.
  ///
  /// # Errors
  /// Fails if the terminal can't be set up, or `Cancelled` if the user pressed Ctrl+C.
  pub fn ask(self) -> Result<usize> {
    let pick = Widget::new(self.options);
    ask(self.prompt, pick, |key| pick.on_key(key), &always_ok)?;

    // `done()` is the only way out other than Ctrl+C, which returns before this.
    Ok(pick.picked().unwrap_or_default())
  }
}
