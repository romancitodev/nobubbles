use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
  app::Inline,
  components::{
    Ask, Render,
    prompt::{Prompt, PromptState},
    text::Text,
  },
  signals::{self, signal},
  style::Style,
};

pub mod confirm;
pub mod input;
pub mod multiselect;
pub mod select;

/// Runs one prompt on the rail until it's submitted, then leaves it drawn as answered.
///
/// That last frame is the transcript: the loop parks the cursor under it, so the next prompt
/// anchors below and the rail joins the two. A widget never has to know what submitting
/// means; it only says whether it wanted the key, and `ask` reads Enter as a yes when it
/// didn't.
pub(crate) fn ask<W: Render + Ask + Copy>(
  title: &str,
  widget: W,
  mut on_key: impl FnMut(KeyEvent) -> bool,
) -> eyre::Result<()> {
  let submitted = signal(false);
  let title = title.to_owned();

  Inline::run(signals::fps(), |cx| {
    if let Some(key) = cx.key() {
      // `ctrl+s` always submits. So does an Enter the widget turned down, and that `bool` is
      // the whole protocol: a multiline `Input` consumes Enter to break the line and stays
      // put, everything else lets it through and ends the prompt (D-015).
      if is_submit(key) || (!on_key(key) && key.code == KeyCode::Enter) {
        submitted.set(true);
        signals::quit();
      }
    }

    // Answered prompts collapse to what was picked. A submitted `Select` that kept its
    // whole list would leave the transcript reading as options rather than as answers.
    if submitted.get() {
      let answer = Text::new(widget.answer()).style(Style::new().dim());
      cx.render(Prompt::new(PromptState::Submitted, title.clone(), answer));
    } else {
      cx.render(Prompt::new(PromptState::Active, title.clone(), widget).hint(widget.controls()));
    }
  })
}

/// The way out that works even when the widget wants Enter for itself. `ctrl+s` and not
/// `ctrl+enter`, which most terminals send as a plain Enter.
fn is_submit(key: KeyEvent) -> bool {
  key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::components::input::Input;

  /// The exact decision `ask` makes, without a terminal in the way.
  fn submits(field: Input, key: KeyEvent) -> bool {
    is_submit(key) || (!field.on_key(key) && key.code == KeyCode::Enter)
  }

  #[test]
  fn enter_submits_a_plain_field_and_breaks_a_multiline_one() {
    assert!(submits(Input::new(), KeyEvent::from(KeyCode::Enter)));

    let field = Input::new().multiline();
    assert!(!submits(field, KeyEvent::from(KeyCode::Enter)));
    assert_eq!(
      field.value(),
      "
",
      "it took the key and broke the line"
    );

    let ctrl_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(submits(field, ctrl_s));
    assert_eq!(
      field.value(),
      "
",
      "and ctrl+s did not type an s"
    );
  }
}
