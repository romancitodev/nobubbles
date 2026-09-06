//! One CLI, three looks.
//!
//! A theme here is data: how the word that opens a line is painted, what the accent is, what
//! a bar is made of. Copy the one you like into your project and change the palette — nothing
//! under `themes()` knows which of them is running.
//!
//! Everything is laid out on one grid: an eight-column gutter for the opening word, two
//! spaces, then the message. Lines, bars and boxes all start in the same place, which is most
//! of what makes a CLI look deliberate rather than assembled.
//!
//! rímel is what makes the astro-style badges possible at all — a solid block behind a word
//! has to *end* where the word does, and a styled row paints to the edge of the terminal.

use std::time::Instant;

use eyre::Result;
use nobubbles::app::Inline;
use nobubbles::rimel::{self, Block, Border, Color, palette};
use nobubbles::signals::{quit, signal};

/// Columns the opening word gets, so every message starts in the same place.
const GUTTER: u16 = 8;
/// Columns a bar takes.
const BAR: u16 = 28;

/// How a theme paints the word that opens a line.
#[derive(Clone, Copy)]
enum Head {
  /// A solid block behind it, the way astro and vite badge theirs.
  Badge { fg: Color, bg: Color },
  /// A marker and some weight. What still reads in a CI log with the colour stripped out.
  Mark(&'static str, Color),
}

struct Theme {
  name: &'static str,
  head: Head,
  accent: Color,
  filled: &'static str,
  unfilled: &'static str,
  border: Border,
}

impl Theme {
  /// The opening word, right-aligned in the gutter.
  ///
  /// The gutter is a spacer next to the badge rather than padding on it: padding takes the
  /// block's background, and a badge that pads left is a badge with a magenta tail.
  fn word(&self, word: &str) -> Block {
    let painted = match self.head {
      Head::Badge { fg, bg } => rimel::text(word).fg(fg).bg(bg).bold().px(1),
      Head::Mark(mark, color) => rimel::text(format!("{mark} {word}")).fg(color).bold(),
    };

    let indent = GUTTER.saturating_sub(painted.size().0);
    rimel::row([rimel::space(indent), painted])
  }

  /// A line of output: the word, then whatever it has to say.
  fn line(&self, word: &str, message: impl AsRef<str>) -> Block {
    rimel::row([self.word(word), rimel::space(2), rimel::text(message)])
  }

  fn say(&self, word: &str, message: impl AsRef<str>) {
    println!("{}", self.line(word, message));
  }

  /// A bar on the same line as its word, so it sits on the same grid as everything else.
  fn bar(&self, word: &str, ratio: f32) -> Block {
    let filled = (f32::from(BAR) * ratio).round().clamp(0.0, f32::from(BAR)) as u16;
    let percent = format!("  {:>3.0}%", ratio * 100.0);

    rimel::row([
      self.word(word),
      rimel::space(2),
      rimel::text(self.filled.repeat(filled as usize)).fg(self.accent),
      rimel::text(self.unfilled.repeat((BAR - filled) as usize)).fg(palette::SURFACE1),
      rimel::text(percent).dim(),
    ])
  }

  /// A box, hanging off the message column.
  fn note(&self, body: &str) {
    let boxed = rimel::text(body)
      .px(1)
      .border_with(self.border)
      .border_color(self.accent);

    println!("{}", rimel::row([rimel::space(GUTTER + 2), boxed]));
  }
}

fn themes() -> [Theme; 3] {
  [
    Theme {
      name: "astro",
      head: Head::Badge {
        fg: palette::BASE,
        bg: palette::MAUVE,
      },
      accent: palette::MAUVE,
      filled: "━",
      unfilled: "━",
      border: Border::rounded(),
    },
    Theme {
      name: "vite",
      head: Head::Badge {
        fg: palette::BASE,
        bg: palette::GREEN,
      },
      accent: palette::GREEN,
      filled: "█",
      unfilled: "░",
      border: Border::thick(),
    },
    Theme {
      name: "ci",
      head: Head::Mark("▸", palette::SAPPHIRE),
      accent: palette::SAPPHIRE,
      filled: "=",
      unfilled: "·",
      border: Border::plain(),
    },
  ]
}

/// A live bar that leaves a *line* behind, not a bar.
///
/// A finished bar in the transcript is a brick: it says "100%" by being a solid rectangle,
/// and a screen of those is what a themed CLI must never end up looking like. The last frame
/// is the one that stays, so the last frame is a sentence.
fn bundle(theme: &Theme) -> Result<()> {
  let started = Instant::now();
  // A signal and not a plain `f32`: writing a signal is what marks the view dirty, and a
  // frame that dirties itself is what keeps the loop drawing. A local leaves the loop idle,
  // and the bar only moves when something else — a keystroke — happens to wake it up.
  let at = signal(0.0f32);

  Inline::run(30, |cx| {
    let ratio = at.update(|ratio| {
      *ratio += 0.02;
      *ratio
    });

    if ratio >= 1.0 {
      let took = started.elapsed().as_secs_f32();
      cx.render(theme.line("build", format!("bundled 12 routes in {took:.1}s")));
      quit();
    } else {
      cx.render(theme.bar("build", ratio));
    }
  })
}

/// The same six lines, whichever theme is handed in.
fn deploy(theme: &Theme) -> Result<()> {
  theme.say(theme.name, "v0.1.0  building for production");
  println!();

  theme.say("build", "12 routes, 3 islands");
  bundle(theme)?;
  println!();

  theme.say("done", "deployed to production");
  println!();
  theme.note("https://nobubbles.dev\n42 files · 1.2 MB");
  println!();

  Ok(())
}

fn main() -> Result<()> {
  for (i, theme) in themes().iter().enumerate() {
    if i > 0 {
      println!("{}", rimel::separator(GUTTER + 2 + BAR + 6).dim().my(1));
    }
    deploy(theme)?;
  }
  Ok(())
}
