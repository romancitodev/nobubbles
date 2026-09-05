use crossterm::event::{KeyCode, KeyEvent};

use crate::{
  app::Inline,
  components::{
    Render,
    prompt::{Prompt, PromptState},
  },
  signals::{self, signal},
};

pub mod confirm;
pub mod input;
pub mod multiselect;
pub mod select;

/// Runs one prompt on the rail until Enter, then leaves it drawn as submitted.
///
/// That last frame is the transcript: the loop parks the cursor under it, so the next prompt
/// anchors below and the rail joins the two. Enter never reaches `on_key`, so a widget never
/// has to know what submitting means.
pub(crate) fn ask<W: Render + Copy>(
  title: &str,
  widget: W,
  mut on_key: impl FnMut(KeyEvent),
) -> eyre::Result<()> {
  let submitted = signal(false);
  let title = title.to_owned();

  Inline::run(signals::fps(), |cx| {
    if let Some(key) = cx.key() {
      if key.code == KeyCode::Enter {
        submitted.set(true);
        signals::quit();
      } else {
        on_key(key);
      }
    }

    let state = if submitted.get() {
      PromptState::Submitted
    } else {
      PromptState::Active
    };

    cx.render(Prompt::new(state, title.clone(), widget));
  })
}
