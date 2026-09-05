//! Lines the session says between questions.
//!
//! A prompt asks something; these don't. They land on the same rail so the transcript reads
//! as one column instead of as prompts with output falling out the side of them.
//!
//! ```no_run
//! # use nobubbles::inline;
//! # let session = inline::intro("Setup")?;
//! inline::log::success("dependencies installed");
//! inline::log::warn("no lockfile, resolving from scratch");
//! # Ok::<(), eyre::Report>(())
//! ```

use std::fmt::Display;
use std::io::Write;

use crossterm::style::Stylize;

use crate::components::prompt::BAR;

/// Neutral. The line just sits on the rail.
pub fn info(message: impl Display) {
  write(BAR.dark_grey().to_string(), message);
}

/// Something finished.
pub fn success(message: impl Display) {
  write("◇".green().to_string(), message);
}

/// Something is worth knowing before it bites.
pub fn warn(message: impl Display) {
  write("▲".yellow().to_string(), message);
}

/// Something went wrong, without ending the session.
pub fn error(message: impl Display) {
  write("■".red().to_string(), message);
}

/// A step that is about to happen, rather than one that already did.
pub fn step(message: impl Display) {
  write("◆".cyan().to_string(), message);
}

/// A titled block: the title on the marker line, the body indented under the rail.
pub fn note(title: impl Display, body: impl Display) {
  write("◇".green().to_string(), title);

  let bar = BAR.dark_grey().to_string();
  for line in body.to_string().lines() {
    line_out(&format!("{bar}  {line}"));
  }
  line_out(&bar);
}

/// Every log line is a marker, a gutter, the message, and a rail line to keep the column
/// going into whatever comes next.
fn write(marker: String, message: impl Display) {
  for (i, line) in message.to_string().lines().enumerate() {
    let prefix = if i == 0 {
      marker.clone()
    } else {
      BAR.dark_grey().to_string()
    };
    line_out(&format!("{prefix}  {line}"));
  }
  line_out(&BAR.dark_grey().to_string());
}

/// `\r\n` and not `println!`: a session holds the terminal in raw mode, where a bare newline
/// drops a row without returning the carriage and the rail comes out as a staircase.
fn line_out(line: &str) {
  let mut out = std::io::stdout();
  let _ = write!(out, "{line}\r\n");
  let _ = out.flush();
}
