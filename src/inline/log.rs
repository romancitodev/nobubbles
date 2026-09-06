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

use crate::components::prompt::BAR;
use crate::rimel;
use crate::style::{Color, palette};

/// A marker painted with the palette.
fn mark(glyph: &str, color: Color) -> String {
  rimel::text(glyph).fg(color).to_string()
}

/// The rail between lines.
fn rail() -> String {
  mark(BAR, palette::SURFACE2)
}

/// Neutral. The line just sits on the rail.
pub fn info(message: impl Display) {
  write(rail(), message);
}

/// Something finished.
pub fn success(message: impl Display) {
  write(mark("◇", palette::GREEN), message);
}

/// Something is worth knowing before it bites.
pub fn warn(message: impl Display) {
  write(mark("▲", palette::PEACH), message);
}

/// Something went wrong, without ending the session.
pub fn error(message: impl Display) {
  write(mark("■", palette::RED), message);
}

/// A step that is about to happen, rather than one that already did.
pub fn step(message: impl Display) {
  write(mark("◆", palette::MAUVE), message);
}

/// A rímel block on the rail.
///
/// Not `println!`: a session holds the terminal in raw mode, where a bare newline drops a row
/// without returning the carriage.
pub fn block(block: &rimel::Block) {
  let bar = rail();
  for line in block.to_string().lines() {
    line_out(&format!("{bar}  {line}"));
  }
  line_out(&bar);
}

/// A titled block: the title on the marker line, the body indented under the rail.
pub fn note(title: impl Display, body: impl Display) {
  write(mark("◇", palette::GREEN), title);

  let bar = rail();
  for line in body.to_string().lines() {
    line_out(&format!("{bar}  {line}"));
  }
  line_out(&bar);
}

/// Every log line is a marker, a gutter, the message, and a rail line to keep the column
/// going into whatever comes next.
fn write(marker: String, message: impl Display) {
  for (i, line) in message.to_string().lines().enumerate() {
    let prefix = if i == 0 { marker.clone() } else { rail() };
    line_out(&format!("{prefix}  {line}"));
  }
  line_out(&rail());
}

/// `\r\n` and not `println!`: a session holds the terminal in raw mode, where a bare newline
/// drops a row without returning the carriage and the rail comes out as a staircase.
fn line_out(line: &str) {
  let mut out = std::io::stdout();
  let _ = write!(out, "{line}\r\n");
  let _ = out.flush();
}
