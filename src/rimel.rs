//! Blocks of styled text, inside a view.
//!
//! The library is [`norimel`] and this re-exports it whole, so a `nobubbles` user gets it
//! without a second line in their manifest. What is added here is the one impl that cannot
//! live over there: [`Render`], which belongs to this crate.
//!
//! ```no_run
//! use nobubbles::rimel::{self, Color};
//!
//! # let mut cx: nobubbles::app::Ctx = unimplemented!();
//! let badge = rimel::text("build").fg(Color::Black).bg(Color::Magenta).px(1);
//! cx.render(rimel::row([badge, rimel::text("  4 routes")]));
//! ```

pub use norimel::*;

use crate::components::{Buffer, Rect, Render};

impl Render for Block {
  fn height(&self, _: u16) -> u16 {
    self.size().1
  }

  fn render(self, area: Rect, buf: &mut Buffer<'_>) {
    let right = area.x().saturating_add(area.width());
    let bottom = area.y().saturating_add(area.height());
    let raw = buf.inner_mut();

    for (x, y, text, style) in self.runs() {
      let x = area.x().saturating_add(x);
      let y = area.y().saturating_add(y);
      if y >= bottom {
        break;
      }
      if x >= right {
        continue;
      }
      // `set_string` clips at the buffer's own edge, so a run that overruns is cut there.
      raw.set_string(x, y, text, ratatui::style::Style::from(style));
    }
  }
}
