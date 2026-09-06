use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
  app::Inline,
  components::{
    Ask, Render,
    prompt::{Prompt, PromptState},
    text::Text,
  },
  rimel::Block,
  signals::{self, signal},
  style::{Style, palette},
};

pub mod autocomplete;
pub mod confirm;
pub mod group_multiselect;
pub mod input;
pub mod multiselect;
pub mod password;
pub mod select;
pub mod select_key;
pub mod task;

/// Runs one prompt on the rail until it's submitted, then leaves it drawn as answered.
///
/// That last frame is the transcript: the loop parks the cursor under it, so the next prompt
/// anchors below and the rail joins the two. A widget never has to know what submitting
/// means; it only says whether it wanted the key, and `ask` reads Enter as a yes when it
/// didn't.
/// A refusal, with the reason to show on the closer.
pub(crate) type Check<'a> = &'a dyn Fn(&str) -> Result<(), String>;

/// A validator a builder is holding on to until it runs.
pub(crate) type Validator<'a> = Box<dyn Fn(&str) -> Result<(), String> + 'a>;

/// Nothing to check. Every prompt without a `validate` uses this.
pub(crate) fn always_ok(_: &str) -> Result<(), String> {
  Ok(())
}

pub(crate) fn ask<W: Render + Ask + Copy>(
  title: impl Into<Block>,
  widget: W,
  mut on_key: impl FnMut(KeyEvent) -> bool,
  check: Check<'_>,
) -> eyre::Result<()> {
  let submitted = signal(false);
  let refused = signal(Option::<String>::None);
  let title = title.into();

  Inline::run(signals::fps(), |cx| {
    if let Some(key) = cx.key() {
      // `ctrl+s` always submits. So does an Enter the widget turned down, and that `bool` is
      // the whole protocol: a multiline `Input` consumes Enter to break the line and stays
      // put, everything else lets it through and ends the prompt (D-015).
      let wanted = on_key(key);
      if is_submit(key) || widget.done() || (!wanted && key.code == KeyCode::Enter) {
        match check(&widget.answer()) {
          Ok(()) => {
            submitted.set(true);
            signals::quit();
          }
          // Refused, not cancelled: the prompt stays live and says why.
          Err(why) => refused.set(Some(why)),
        }
      } else {
        // Any edit clears the complaint, so it doesn't outlive what caused it.
        refused.set(None);
      }
    }

    // Answered prompts collapse to what was picked. A submitted `Select` that kept its
    // whole list would leave the transcript reading as options rather than as answers.
    if submitted.get() {
      let answer = Text::new(widget.answer()).style(Style::new().dim());
      let title = title.clone().settled(palette::OVERLAY1);
      cx.render(Prompt::new(PromptState::Submitted, title, answer));
    } else if let Some(why) = refused.get() {
      cx.render(Prompt::new(PromptState::Error, title.clone(), widget).hint(why));
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
