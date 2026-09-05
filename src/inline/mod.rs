//! Sequential inline prompts: ask one thing, get a value back, move on.
//!
//! Each prompt is an [`Inline::run`](crate::app::Inline::run) that ends when the user hits
//! Enter, so there's a single interactive widget alive at a time and the whole question of
//! which widget has focus never comes up. For a *live* inline view (a spinner, a progress
//! bar, three lines updating on their own) reach for `Inline::run` directly instead.

mod components;

pub use components::confirm::confirm;
pub use components::input::input;
pub use components::multiselect::multiselect;
pub use components::select::select;

use crossterm::style::Stylize;

use crate::components::prompt::{BAR, BAR_END, BAR_START};

/// Keeps the terminal in raw mode for as long as it's alive.
///
/// Guards nest and only the outermost one toggles anything. Every `Inline::run` takes one,
/// so a lone `input()` works on its own; [`intro`] takes one that outlives a whole batch, and
/// then raw mode never drops between prompts. That gap is not cosmetic — with echo back on,
/// anything typed between two prompts goes to the shell's line buffer and gets printed over
/// the UI.
///
/// Being a `Drop` also means a panic mid-prompt can't strand the terminal in raw mode.
pub struct Session;

impl Session {
  pub(crate) fn open() -> eyre::Result<Self> {
    if crate::signals::raw_enter() {
      crossterm::terminal::enable_raw_mode()?;
    }
    Ok(Self)
  }

  /// Sets the frame rate for the prompts run under this session. Defaults to 30.
  ///
  /// It's how long a burst of keystrokes gets coalesced before a repaint, not how often the
  /// screen refreshes — an idle prompt repaints nothing at all. Raise it if held keys feel
  /// laggy, lower it if a paste of a long line makes the terminal work too hard.
  #[must_use]
  pub fn fps(self, fps: u16) -> Self {
    crate::signals::set_fps(fps);
    self
  }
}

impl Drop for Session {
  fn drop(&mut self) {
    if crate::signals::raw_exit() {
      let _ = crossterm::terminal::disable_raw_mode();
      crate::signals::reset_fps();
    }
  }
}

/// Opens a prompt session, printing `title` above it, and hands back the guard that keeps it
/// open. Hold it for as long as you're prompting:
///
/// ```no_run
/// use nobubbles::inline;
///
/// let session = inline::intro("Config")?.fps(60);
/// let name = inline::input("What's your name?")?;
/// inline::outro(session).with("Done");
/// # Ok::<(), eyre::Report>(())
/// ```
///
/// Calling a prompt without one still works — it just opens and closes its own.
///
/// # Errors
/// Can return an error if the terminal cannot be initialized or if there is an issue with rendering
pub fn intro(title: &str) -> eyre::Result<Session> {
  // Printed before raw mode goes on, so the newlines still return the carriage and the first
  // prompt anchors its viewport on the line below. The trailing bar is what the first prompt
  // hangs off.
  println!("{}  {title}", BAR_START.dark_grey());
  println!("{}", BAR.dark_grey());
  Session::open()
}

/// Closes a prompt session. Raw mode goes off here, so anything printed afterwards behaves
/// like normal program output again, a bare newline returns the carriage.
///
/// On its own it just closes. Chain [`Outro::with`] to sign off with a line under the
/// transcript:
///
/// ```no_run
/// # let session = nobubbles::inline::intro("Config")?;
/// nobubbles::inline::outro(session).with("Done");
/// # Ok::<(), eyre::Report>(())
/// ```
///
#[must_use]
pub fn outro(session: Session) -> Outro {
  drop(session);
  Outro
}

/// What [`outro`] hands back, so the closing line stays optional without an `Option`.
pub struct Outro;

impl Outro {
  /// Closes the rail with a message. Extra lines are indented under it rather than getting
  /// their own corner, since a rail only ends once.
  pub fn with(self, message: impl std::fmt::Display) {
    let message = message.to_string();
    let mut lines = message.lines();

    println!(
      "{}  {}",
      BAR_END.dark_grey(),
      lines.next().unwrap_or_default()
    );
    for line in lines {
      println!("   {line}");
    }
  }
}
