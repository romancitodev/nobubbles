//! rímel — blocks of styled text, lipgloss style.
//!
//! A [`Block`] is rows of styled runs plus tailwind-ish utilities: `bg`, `px`, `w`, `center`,
//! `rounded`. The utilities only take notes; the shape is composed once at the end, so they
//! commute and calling one twice doesn't stack.
//!
//! ```
//! use norimel::{self as rimel, Color};
//!
//! let badge = rimel::text("astro").bg(Color::Magenta).fg(Color::Black).bold().px(1);
//! println!("{}", rimel::row([badge, rimel::text("  ready in 300ms")]));
//! ```
//!
//! Two ways out of the same block: [`Display`](std::fmt::Display) writes ANSI for a
//! `println!`, [`Block::runs`] hands the placed runs to whoever paints cells. Runs are never
//! flattened into a string, so measuring and joining never parses ANSI back out.

use std::fmt;
#[cfg(feature = "gradient")]
use std::sync::OnceLock;
#[cfg(feature = "gradient")]
use std::time::Instant;

use crossterm::style::StyledContent;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub mod style;

pub use style::{Color, Style, palette};

/// Where a row sits when the block is wider than it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Align {
  #[default]
  Left,
  Center,
  Right,
}

/// Where a block sits when the row of blocks is taller than it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VAlign {
  #[default]
  Top,
  Middle,
  Bottom,
}

/// A colour ramp, for [`Block::gradient`] and [`Block::animate`].
///
/// A thin wrapper over a [`colorgrad`] gradient, so anything that crate can build works here:
/// the presets below, `GradientBuilder` stops, css strings.
#[cfg(feature = "gradient")]
#[derive(Clone)]
pub struct Ramp {
  inner: Box<dyn colorgrad::Gradient>,
  /// How far each colour is blended toward white.
  wash: f32,
}

#[cfg(feature = "gradient")]
impl Ramp {
  /// Any colorgrad gradient.
  pub fn new(gradient: impl colorgrad::Gradient + 'static) -> Self {
    Self {
      inner: Box::new(gradient),
      wash: 0.0,
    }
  }

  /// The hue wheel.
  #[must_use]
  pub fn rainbow() -> Self {
    Self::new(colorgrad::preset::rainbow())
  }

  /// Sine-based rainbow: even brightness, no dark band where it wraps.
  #[must_use]
  pub fn sinebow() -> Self {
    Self::new(colorgrad::preset::sinebow())
  }

  /// The wheel washed out.
  #[must_use]
  pub fn pastel() -> Self {
    Self::sinebow().wash(0.45)
  }

  #[must_use]
  pub fn turbo() -> Self {
    Self::new(colorgrad::preset::turbo())
  }

  #[must_use]
  pub fn cool() -> Self {
    Self::new(colorgrad::preset::cool())
  }

  #[must_use]
  pub fn warm() -> Self {
    Self::new(colorgrad::preset::warm())
  }

  /// Blends every colour `amount` of the way to white. What makes a palette pastel.
  #[must_use]
  pub fn wash(mut self, amount: f32) -> Self {
    self.wash = amount.clamp(0.0, 1.0);
    self
  }

  /// The colour at `t`. Outside 0..1 it repeats, which is what lets an animation drift.
  #[must_use]
  pub fn at(&self, t: f32) -> Color {
    use colorgrad::Gradient as _;

    let [r, g, b, _] = self.inner.repeat_at(t).to_rgba8();
    let wash = |channel: u8| {
      let channel = f32::from(channel);
      (channel + (255.0 - channel) * self.wash) as u8
    };
    Color::Rgb(wash(r), wash(g), wash(b))
  }
}

#[cfg(feature = "gradient")]
impl fmt::Debug for Ramp {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Ramp").field("wash", &self.wash).finish()
  }
}

/// Seconds since the first animated block was composed.
#[cfg(feature = "gradient")]
fn clock() -> f32 {
  static START: OnceLock<Instant> = OnceLock::new();
  START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// The glyphs of a box, for [`Block::border_with`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Border {
  pub horizontal: &'static str,
  pub vertical: &'static str,
  pub top_left: &'static str,
  pub top_right: &'static str,
  pub bottom_left: &'static str,
  pub bottom_right: &'static str,
}

impl Border {
  /// Square corners, light lines.
  #[must_use]
  pub fn plain() -> Self {
    Self {
      horizontal: "─",
      vertical: "│",
      top_left: "┌",
      top_right: "┐",
      bottom_left: "└",
      bottom_right: "┘",
    }
  }

  /// Square corners, heavy lines.
  #[must_use]
  pub fn thick() -> Self {
    Self {
      horizontal: "━",
      vertical: "┃",
      top_left: "┏",
      top_right: "┓",
      bottom_left: "┗",
      bottom_right: "┛",
    }
  }

  /// Round corners.
  #[must_use]
  pub fn rounded() -> Self {
    Self {
      top_left: "╭",
      top_right: "╮",
      bottom_left: "╰",
      bottom_right: "╯",
      ..Self::plain()
    }
  }
}

/// A stretch of text painted one way.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Run {
  text: String,
  style: Style,
}

impl Run {
  fn new(text: impl Into<String>, style: Style) -> Self {
    Self {
      text: text.into(),
      style,
    }
  }

  fn space(columns: u16, style: Style) -> Self {
    Self::new(" ".repeat(columns as usize), style)
  }

  fn width(&self) -> u16 {
    width_of(&self.text)
  }
}

/// A rectangle of styled text.
#[derive(Clone, Debug, Default)]
pub struct Block {
  /// The content only. Padding, border and alignment slack are added by `compose`.
  rows: Vec<Vec<Run>>,
  /// The block's style. A run that leaves a colour unsaid inherits this one.
  style: Style,
  /// Rows, columns.
  pad: (u16, u16),
  margin: (u16, u16),
  /// Content width. `None` measures it.
  width: Option<u16>,
  align: Align,
  border: Option<(Border, Style)>,
  /// The ramp, and how many turns per second it drifts. Zero is a still gradient.
  #[cfg(feature = "gradient")]
  gradient: Option<(Ramp, f32)>,
}

/// Two blocks are equal when they compose to the same thing.
impl PartialEq for Block {
  fn eq(&self, other: &Self) -> bool {
    self.compose() == other.compose()
  }
}

/// A block from a string. Newlines split rows.
///
/// An empty string is one empty row, not zero: otherwise it would vanish out of a [`col`].
pub fn text(content: impl AsRef<str>) -> Block {
  let content = content.as_ref();
  let rows = if content.is_empty() {
    vec![Vec::new()]
  } else {
    content
      .lines()
      .map(|line| vec![Run::new(line, Style::new())])
      .collect()
  };

  Block {
    rows,
    ..Block::default()
  }
}

/// A horizontal rule, `columns` wide.
#[must_use]
pub fn separator(columns: u16) -> Block {
  text("─".repeat(columns as usize))
}

/// Blank columns, to push the next block along a row. `w-8` with nothing in it.
///
/// A spacer, not padding on the neighbour: padding is painted with that block's background.
#[must_use]
pub fn space(columns: u16) -> Block {
  text(" ".repeat(columns as usize))
}

impl Block {
  /// `text-black`.
  #[must_use]
  pub fn fg(mut self, color: Color) -> Self {
    self.style.fg = color;
    self
  }

  /// `bg-magenta`. Reaches as far as the block does, padding and slack included.
  #[must_use]
  pub fn bg(mut self, color: Color) -> Self {
    self.style.bg = color;
    self
  }

  /// `font-bold`.
  #[must_use]
  pub fn bold(mut self) -> Self {
    self.style.bold = true;
    self
  }

  /// `opacity-60`, or whatever the terminal does with a dim attribute.
  #[must_use]
  pub fn dim(mut self) -> Self {
    self.style.dim = true;
    self
  }

  /// `italic`.
  #[must_use]
  pub fn italic(mut self) -> Self {
    self.style.italic = true;
    self
  }

  /// All of the above at once, for a style that already lives in a theme.
  #[must_use]
  pub fn style(mut self, style: Style) -> Self {
    self.style = style.over(self.style);
    self
  }

  /// One colour per character, taken across the ramp.
  #[cfg(feature = "gradient")]
  #[must_use]
  pub fn gradient(mut self, ramp: Ramp) -> Self {
    self.gradient = Some((ramp, 0.0));
    self
  }

  /// A gradient that drifts `speed` turns per second.
  ///
  /// rímel has no loop: it reads the clock while composing, so this moves for as long as
  /// something keeps redrawing.
  #[cfg(feature = "gradient")]
  #[must_use]
  pub fn animate(mut self, ramp: Ramp, speed: f32) -> Self {
    self.gradient = Some((ramp, speed));
    self
  }

  /// `p-2`. Inside the block, so the background covers it.
  #[must_use]
  pub fn p(self, cells: u16) -> Self {
    self.py(cells).px(cells)
  }

  /// `px-2`.
  #[must_use]
  pub fn px(mut self, cells: u16) -> Self {
    self.pad.1 = cells;
    self
  }

  /// `py-1`.
  #[must_use]
  pub fn py(mut self, cells: u16) -> Self {
    self.pad.0 = cells;
    self
  }

  /// `m-2`. Outside the block, so it keeps the terminal's own colour.
  #[must_use]
  pub fn m(self, cells: u16) -> Self {
    self.my(cells).mx(cells)
  }

  /// `mx-2`.
  #[must_use]
  pub fn mx(mut self, cells: u16) -> Self {
    self.margin.1 = cells;
    self
  }

  /// `my-1`.
  #[must_use]
  pub fn my(mut self, cells: u16) -> Self {
    self.margin.0 = cells;
    self
  }

  /// `w-40`, over the text: padding and border go outside it.
  ///
  /// Short rows are filled, long ones are cut. Cut, not wrapped, for now.
  #[must_use]
  pub fn w(mut self, columns: u16) -> Self {
    self.width = Some(columns);
    self
  }

  /// `text-left`.
  #[must_use]
  pub fn left(mut self) -> Self {
    self.align = Align::Left;
    self
  }

  /// `text-center`.
  #[must_use]
  pub fn center(mut self) -> Self {
    self.align = Align::Center;
    self
  }

  /// `text-right`.
  #[must_use]
  pub fn right(mut self) -> Self {
    self.align = Align::Right;
    self
  }

  /// A box around it, square corners.
  #[must_use]
  pub fn border(self) -> Self {
    self.border_with(Border::plain())
  }

  /// Round corners. Adds a box if there was none.
  #[must_use]
  pub fn rounded(self) -> Self {
    self.border_with(Border::rounded())
  }

  /// Heavy lines. Adds a box if there was none.
  #[must_use]
  pub fn thick(self) -> Self {
    self.border_with(Border::thick())
  }

  /// A box out of your own glyphs.
  #[must_use]
  pub fn border_with(mut self, border: Border) -> Self {
    let style = self.border.map_or_else(Style::new, |(_, style)| style);
    self.border = Some((border, style));
    self
  }

  /// `border-magenta`. Adds a box if there was none.
  #[must_use]
  pub fn border_color(mut self, color: Color) -> Self {
    let (border, style) = self.border.unwrap_or((Border::plain(), Style::new()));
    self.border = Some((border, style.fg(color)));
    self
  }

  /// Columns and rows it takes, everything the utilities add included.
  #[must_use]
  pub fn size(&self) -> (u16, u16) {
    let rows = self.compose();
    let width = rows.iter().map(|row| row_width(row)).max().unwrap_or(0);
    (width, u16::try_from(rows.len()).unwrap_or(u16::MAX))
  }

  /// The placed runs: `(column, row, text, style)`, relative to the block.
  ///
  /// They don't overlap and come in order, so writing them as they arrive is enough.
  pub fn runs(&self) -> impl Iterator<Item = (u16, u16, String, Style)> {
    self
      .compose()
      .into_iter()
      .enumerate()
      .flat_map(|(row, runs)| {
        let y = u16::try_from(row).unwrap_or(u16::MAX);
        runs.into_iter().scan(0u16, move |x, run| {
          let at = *x;
          *x = x.saturating_add(run.width());
          Some((at, y, run.text, run.style))
        })
      })
  }

  /// Turns the utilities into rows of runs. Every way out goes through here.
  fn compose(&self) -> Vec<Vec<Run>> {
    let mut rows: Vec<Vec<Run>> = self
      .rows
      .iter()
      .map(|row| {
        row
          .iter()
          .map(|run| Run::new(run.text.clone(), run.style.over(self.style)))
          .collect()
      })
      .collect();

    // Before the padding: the ramp belongs to the text, not to the box around it.
    #[cfg(feature = "gradient")]
    if let Some((ramp, speed)) = &self.gradient {
      let phase = if *speed == 0.0 { 0.0 } else { clock() * speed };
      for row in &mut rows {
        *row = spread(row, ramp, phase);
      }
    }

    let content = self.width.unwrap_or_else(|| {
      rows
        .iter()
        .map(|row| row_width(row))
        .max()
        .unwrap_or_default()
    });
    for row in &mut rows {
      fit(row, content, self.align, self.style);
    }

    let (py, px) = self.pad;
    let padded = frame(&mut rows, content, py, px, self.style);

    let boxed = match self.border {
      None => padded,
      Some((border, style)) => {
        for row in &mut rows {
          row.insert(0, Run::new(border.vertical, style));
          row.push(Run::new(border.vertical, style));
        }
        let bar = |left: &str, right: &str| {
          let line = format!("{left}{}{right}", border.horizontal.repeat(padded as usize));
          vec![Run::new(line, style)]
        };
        rows.insert(0, bar(border.top_left, border.top_right));
        rows.push(bar(border.bottom_left, border.bottom_right));
        padded.saturating_add(2)
      }
    };

    let (my, mx) = self.margin;
    frame(&mut rows, boxed, my, mx, Style::new());
    rows
  }
}

/// One run per grapheme, each with its own point on the ramp.
#[cfg(feature = "gradient")]
fn spread(row: &[Run], ramp: &Ramp, phase: f32) -> Vec<Run> {
  let total = f32::from(row_width(row).max(1));
  let mut out = Vec::new();
  let mut at = 0u16;

  for run in row {
    for grapheme in run.text.graphemes(true) {
      let color = ramp.at(f32::from(at) / total + phase);
      out.push(Run::new(grapheme, run.style.fg(color)));
      at = at.saturating_add(width_of(grapheme));
    }
  }
  out
}

/// Wraps `rows` in blank rows and columns of `style`, and answers the resulting width.
/// Padding and margin are the same move with a different style.
fn frame(rows: &mut Vec<Vec<Run>>, width: u16, vertical: u16, horizontal: u16, style: Style) -> u16 {
  if horizontal > 0 {
    let side = Run::space(horizontal, style);
    for row in rows.iter_mut() {
      row.insert(0, side.clone());
      row.push(side.clone());
    }
  }

  let width = width.saturating_add(horizontal.saturating_mul(2));
  for _ in 0..vertical {
    rows.insert(0, vec![Run::space(width, style)]);
    rows.push(vec![Run::space(width, style)]);
  }
  width
}

/// Leaves a row at exactly `width` columns: cut what overruns, fill what falls short.
fn fit(row: &mut Vec<Run>, width: u16, align: Align, style: Style) {
  if row_width(row) > width {
    truncate(row, width);
  }

  // Measured again: a row cut on a double-width glyph gives up two columns to keep one, and
  // that column still belongs to the block.
  let slack = width.saturating_sub(row_width(row));
  let (left, right) = match align {
    Align::Left => (0, slack),
    Align::Center => (slack / 2, slack - slack / 2),
    Align::Right => (slack, 0),
  };

  if left > 0 {
    row.insert(0, Run::space(left, style));
  }
  if right > 0 {
    row.push(Run::space(right, style));
  }
}

/// Blocks side by side, top-aligned. `flex-row`.
#[must_use]
pub fn row(blocks: impl IntoIterator<Item = Block>) -> Block {
  row_items(VAlign::Top, blocks)
}

/// [`row`], saying where the shorter blocks sit. `items-center`, `items-end`.
#[must_use]
pub fn row_items(align: VAlign, blocks: impl IntoIterator<Item = Block>) -> Block {
  let mut pieces: Vec<Vec<Vec<Run>>> = blocks.into_iter().map(|block| block.compose()).collect();
  let height = pieces.iter().map(Vec::len).max().unwrap_or(0);

  for piece in &mut pieces {
    let width = piece.iter().map(|row| row_width(row)).max().unwrap_or(0);
    let missing = height - piece.len();
    let (top, bottom) = match align {
      VAlign::Top => (0, missing),
      VAlign::Middle => (missing / 2, missing - missing / 2),
      VAlign::Bottom => (missing, 0),
    };

    // Squared off against its own widest row, or a ragged piece drags the next one out.
    for row in piece.iter_mut() {
      fit(row, width, Align::Left, Style::new());
    }
    let blank = vec![Run::space(width, Style::new())];
    for _ in 0..top {
      piece.insert(0, blank.clone());
    }
    piece.extend(std::iter::repeat_n(blank, bottom));
  }

  let mut rows = vec![Vec::new(); height];
  for piece in pieces {
    for (row, added) in rows.iter_mut().zip(piece) {
      row.extend(added);
    }
  }

  Block {
    rows,
    ..Block::default()
  }
}

/// Blocks stacked, squared off against the widest. `flex-col`.
#[must_use]
pub fn col(blocks: impl IntoIterator<Item = Block>) -> Block {
  col_items(Align::Left, blocks)
}

/// [`col`], saying where the narrower blocks sit.
#[must_use]
pub fn col_items(align: Align, blocks: impl IntoIterator<Item = Block>) -> Block {
  let mut pieces: Vec<Vec<Vec<Run>>> = blocks.into_iter().map(|block| block.compose()).collect();
  let width = pieces
    .iter()
    .flat_map(|piece| piece.iter().map(|row| row_width(row)))
    .max()
    .unwrap_or(0);

  let mut rows = Vec::new();
  for piece in &mut pieces {
    for row in piece.iter_mut() {
      fit(row, width, align, Style::new());
    }
    rows.append(piece);
  }

  Block {
    rows,
    ..Block::default()
  }
}

/// Columns a string takes on screen. Neither bytes nor chars: a CJK glyph is two.
#[must_use]
pub fn width_of(text: &str) -> u16 {
  u16::try_from(UnicodeWidthStr::width(text)).unwrap_or(u16::MAX)
}

fn row_width(row: &[Run]) -> u16 {
  row
    .iter()
    .fold(0u16, |total, run| total.saturating_add(run.width()))
}

/// Cuts a row to `width` columns, by whole runs and then by graphemes.
fn truncate(row: &mut Vec<Run>, width: u16) {
  let mut left = width;
  let mut kept = Vec::new();

  for run in row.drain(..) {
    if left == 0 {
      break;
    }

    let run_width = run.width();
    if run_width <= left {
      left -= run_width;
      kept.push(run);
      continue;
    }

    let mut text = String::new();
    let mut used = 0;
    for grapheme in run.text.graphemes(true) {
      let grapheme_width = width_of(grapheme);
      if used + grapheme_width > left {
        break;
      }
      used += grapheme_width;
      text.push_str(grapheme);
    }
    kept.push(Run { text, ..run });
    break;
  }

  *row = kept;
}

impl fmt::Display for Block {
  /// ANSI, one row per line. A bare `\n`: under raw mode the caller adds its own `\r`.
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (i, row) in self.compose().iter().enumerate() {
      if i > 0 {
        f.write_str("\n")?;
      }
      for run in row {
        write!(f, "{}", StyledContent::new(run.style.into(), &run.text))?;
      }
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The composed rows without the styles: only what the utilities did to the shape.
  fn plain(block: &Block) -> Vec<String> {
    block
      .compose()
      .iter()
      .map(|row| row.iter().map(|run| run.text.as_str()).collect())
      .collect()
  }

  #[test]
  fn a_block_is_its_lines_and_an_empty_string_is_still_a_row() {
    assert_eq!(text("one\ntwo").size(), (3, 2));
    assert_eq!(text("").size(), (0, 1));
    assert_eq!(text("wide\nx").size(), (4, 2));
  }

  #[test]
  fn w_fills_short_rows_and_cuts_long_ones() {
    assert_eq!(plain(&text("ab\nabcdef").w(4)), ["ab  ", "abcd"]);
  }

  /// Why the utilities only take notes: any order gives the same block.
  #[test]
  fn the_utilities_commute() {
    let one = text("ab").w(6).center().px(1);
    let other = text("ab").px(1).center().w(6);

    assert_eq!(plain(&one), ["   ab   "]);
    assert_eq!(one, other);
  }

  #[test]
  fn a_utility_replaces_itself_instead_of_stacking() {
    assert_eq!(plain(&text("x").px(2).px(1)), [" x "]);
  }

  #[test]
  fn align_puts_the_slack_on_the_other_side() {
    assert_eq!(plain(&text("ab").w(5).right()), ["   ab"]);
    assert_eq!(plain(&text("ab").w(5).left()), ["ab   "]);
  }

  #[test]
  fn padding_is_inside_the_border_and_margin_is_outside_it() {
    let block = text("hi").px(1).border().mx(2);

    assert_eq!(plain(&block), ["  ┌────┐  ", "  │ hi │  ", "  └────┘  "]);
    assert_eq!(block.size(), (10, 3));
  }

  /// What makes a badge a badge: the background reaches the padding and stops at the margin.
  #[test]
  fn the_background_fills_the_padding_and_not_the_margin() {
    let badge = text("go").bg(Color::Magenta).px(1).mx(1);
    let painted: Vec<Color> = badge.runs().map(|(.., style)| style.bg).collect();

    assert_eq!(
      painted,
      [
        Color::Reset,
        Color::Magenta,
        Color::Magenta,
        Color::Magenta,
        Color::Reset
      ]
    );
  }

  #[test]
  fn a_border_wraps_the_widest_row_and_takes_a_colour() {
    let block = text("hi\nthere").rounded().border_color(Color::Cyan);

    assert_eq!(plain(&block), ["╭─────╮", "│hi   │", "│there│", "╰─────╯"]);
    assert_eq!(block.runs().next().unwrap().3.fg, Color::Cyan);
  }

  #[test]
  fn a_row_squares_the_pieces_off_before_it_zips_them() {
    let left = text("a\nbb");
    let right = text("X");

    assert_eq!(plain(&row([left.clone(), right.clone()])), ["a X", "bb "]);
    assert_eq!(
      plain(&row_items(VAlign::Bottom, [left, right])),
      ["a  ", "bbX"]
    );
  }

  #[test]
  fn a_col_squares_everything_to_the_widest_block() {
    assert_eq!(
      plain(&col_items(Align::Center, [text("title"), text("x")])),
      ["title", "  x  "]
    );
  }

  /// A join keeps what each piece was instead of flattening it to one colour.
  #[test]
  fn a_join_bakes_the_styles_of_its_pieces() {
    let joined = row([text("hot").bg(Color::Red), text("cold")]);
    let backgrounds: Vec<Color> = joined.runs().map(|(.., style)| style.bg).collect();

    assert_eq!(backgrounds, [Color::Red, Color::Reset]);
  }

  #[test]
  fn a_separator_is_a_rule_the_utilities_still_apply_to() {
    assert_eq!(plain(&separator(4)), ["────"]);
    assert_eq!(plain(&separator(3).my(1)), ["   ", "───", "   "]);
  }

  /// A gradient is one run per character, and the ramp walks along the row.
  #[cfg(feature = "gradient")]
  #[test]
  fn a_gradient_paints_every_character_on_its_own() {
    let block = text("abcd").gradient(Ramp::rainbow());
    let colors: Vec<Color> = block.runs().map(|(.., style)| style.fg).collect();

    assert_eq!(colors.len(), 4);
    assert_eq!(colors[0], Ramp::rainbow().at(0.0));
    assert_eq!(colors[2], Ramp::rainbow().at(0.5));
    assert!(colors[0] != colors[1]);
  }

  /// Pastel is the same wheel washed out: no channel ever goes near black.
  #[cfg(feature = "gradient")]
  #[test]
  fn pastel_stays_light() {
    for step in 0u8..12 {
      let Color::Rgb(r, g, b) = Ramp::pastel().at(f32::from(step) / 12.0) else {
        panic!("a ramp is always rgb");
      };
      assert!(u16::from(r) + u16::from(g) + u16::from(b) > 450, "{r} {g} {b}");
    }
  }

  /// The ramp colours the text and stops there: padding keeps the block's background.
  #[cfg(feature = "gradient")]
  #[test]
  fn a_gradient_does_not_reach_the_padding() {
    let block = text("ab").gradient(Ramp::rainbow()).px(1);
    let colors: Vec<Color> = block.runs().map(|(.., style)| style.fg).collect();

    assert_eq!(colors[0], Color::Reset);
    assert_eq!(colors[3], Color::Reset);
  }

  #[test]
  fn a_wide_grapheme_counts_for_two_columns() {
    assert_eq!(width_of("日本"), 4);
    assert_eq!(plain(&text("日本").w(3)), ["日 "]);
  }

  #[test]
  fn display_writes_escapes_only_where_there_is_a_style() {
    assert_eq!(text("bare").to_string(), "bare");

    let painted = text("hot").bg(Color::Magenta).to_string();
    assert!(painted.contains("hot"));
    assert!(painted.starts_with('\u{1b}'), "{painted:?}");
  }

  #[test]
  fn runs_come_out_placed_row_by_row() {
    let placed: Vec<_> = row([text("go").px(1), text(" ready")])
      .runs()
      .map(|(x, y, text, _)| (x, y, text))
      .collect();

    assert_eq!(
      placed,
      [
        (0, 0, " ".to_owned()),
        (1, 0, "go".to_owned()),
        (3, 0, " ".to_owned()),
        (4, 0, " ready".to_owned()),
      ]
    );
  }
}
