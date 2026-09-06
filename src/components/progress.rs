use ratatui::{
  text::{Line, Span},
  widgets::{LineGauge, Paragraph, Widget},
};

use crate::{
  components::Render,
  signals::{Signal, signal},
  style::{Style, palette},
};

/// Spinner frames for a progress with no ratio yet.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The painted parts of a [`Progress`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProgressStyle {
  pub filled: Style,
  pub unfilled: Style,
  pub label: Style,
  pub filled_symbol: &'static str,
  pub unfilled_symbol: &'static str,
  /// Widest the row is allowed to draw, in columns. `None` takes whatever it's given.
  pub width: Option<u16>,
}

impl Default for ProgressStyle {
  fn default() -> Self {
    Self {
      filled: Style::new().fg(palette::MAUVE),
      unfilled: Style::new().fg(palette::SURFACE1),
      label: Style::new(),
      filled_symbol: "━",
      unfilled_symbol: "━",
      width: None,
    }
  }
}

/// One row of progress: a label plus an optional fraction.
///
/// With no fraction it spins; with one it draws a bar. Several jobs running at once are
/// several `Progress` in a [`column!`](crate::column), not a different type.
#[derive(Clone, Copy)]
pub struct Progress {
  label: Signal<String>,
  ratio: Signal<Option<f32>>,
  style: Signal<ProgressStyle>,
  spinner: Signal<usize>,
}

impl Progress {
  /// An unlabelled, spinning progress.
  pub fn new() -> Self {
    Self::with("")
  }

  /// A spinning progress with a label. Call [`Progress::set`] to turn it into a bar.
  pub fn with(label: impl Into<String>) -> Self {
    Self {
      label: signal(label.into()),
      ratio: signal(None),
      style: signal(ProgressStyle::default()),
      spinner: signal(0),
    }
  }

  /// Replaces the style.
  #[must_use]
  pub fn style(self, style: ProgressStyle) -> Self {
    self.style.set(style);
    self
  }

  /// Caps how wide the row draws. Without it the bar stretches to the whole terminal, which
  /// reads as noise once there's more than one of them.
  #[must_use]
  pub fn width(self, columns: u16) -> Self {
    self.style.update(|style| style.width = Some(columns));
    self
  }

  /// Sets how far along this is, from 0.0 to 1.0. Out of range values are clamped, since
  /// the bar underneath panics on them.
  pub fn set(&self, ratio: f32) {
    self.ratio.set(Some(ratio.clamp(0.0, 1.0)));
    self.tick();
  }

  /// Replaces the label. Reporting a new one counts as progress, so the spinner moves.
  pub fn set_label(&self, label: impl Into<String>) {
    self.label.set(label.into());
    self.tick();
  }

  /// The text next to the bar.
  pub fn label(&self) -> String {
    self.label.get()
  }

  /// How far along this is, or `None` while it's still indeterminate.
  pub fn ratio(&self) -> Option<f32> {
    self.ratio.get()
  }

  /// Whether the bar reached the end. An indeterminate progress is never done.
  pub fn is_done(&self) -> bool {
    self.ratio.get().is_some_and(|r| r >= 1.0)
  }

  /// Moves the spinner one frame on.
  ///
  /// [`Progress::set`] and [`Progress::set_label`] already do this, so most code never calls
  /// it. Reach for it when a job is alive but has nothing new to say: the spinner is driven
  /// by news, not by a timer, so a job that stops reporting freezes rather than pinning the
  /// loop at the frame rate forever.
  pub fn tick(&self) {
    self.spinner.update(|frame| *frame = frame.wrapping_add(1));
  }
}

impl Default for Progress {
  fn default() -> Self {
    Self::new()
  }
}

impl Render for Progress {
  fn render(self, area: super::Rect, buf: &mut super::Buffer<'_>) {
    let style = self.style.get();
    let label = self.label.get();

    let mut area: ratatui::layout::Rect = area.into();
    if let Some(columns) = style.width {
      area.width = area.width.min(columns);
    }

    match self.ratio.get() {
      Some(ratio) => {
        let bar = LineGauge::default()
          .ratio(f64::from(ratio))
          .label(Line::styled(label, style.label))
          .filled_style(style.filled)
          .unfilled_style(style.unfilled)
          .filled_symbol(style.filled_symbol)
          .unfilled_symbol(style.unfilled_symbol);
        Widget::render(bar, area, buf.inner_mut());
      }
      None => {
        let frame = SPINNER[self.spinner.get() % SPINNER.len()];
        let line = Line::from(vec![
          Span::styled(frame, style.filled),
          Span::raw(" "),
          Span::styled(label, style.label),
        ]);
        Widget::render(Paragraph::new(line), area, buf.inner_mut());
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use ratatui::buffer::Buffer as RatatuiBuffer;

  use super::*;

  fn green() -> ratatui::style::Style {
    ProgressStyle::default().filled.into()
  }

  fn gray() -> ratatui::style::Style {
    ProgressStyle::default().unfilled.into()
  }

  fn expected(spans: impl IntoIterator<Item = Span<'static>>) -> RatatuiBuffer {
    RatatuiBuffer::with_lines([Line::from(spans.into_iter().collect::<Vec<_>>())])
  }

  fn draw(job: Progress) -> ratatui::Terminal<ratatui::backend::TestBackend> {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(10, 1)).unwrap();
    terminal
      .draw(|frame| {
        let area = frame.area().into();
        let mut buf = crate::components::Buffer::from(frame.buffer_mut());
        job.render(area, &mut buf);
      })
      .unwrap();
    terminal
  }

  #[test]
  fn set_clamps_and_reports_done() {
    let job = Progress::with("linking");
    assert_eq!(job.ratio(), None, "a fresh progress is indeterminate");
    assert!(!job.is_done(), "and an indeterminate one is never done");

    job.set(-1.0);
    assert_eq!(job.ratio(), Some(0.0));

    job.set(0.5);
    assert!(!job.is_done());

    job.set(1.5); // LineGauge::ratio panics past 1.0
    assert_eq!(job.ratio(), Some(1.0));
    assert!(job.is_done());
  }

  /// The whole draw path, with no terminal involved: half the row filled at 0.5.
  #[test]
  fn a_half_done_bar_fills_half_the_row() {
    // Distinct symbols so the split shows up in the cells, not only in the colors.
    let job = Progress::new().style(ProgressStyle {
      filled_symbol: "#",
      unfilled_symbol: ".",
      ..ProgressStyle::default()
    });
    job.set(0.5);
    let terminal = draw(job);

    // The leading blank is the empty label, `LineGauge` always reserves it a slot. The
    // colors are the point: they come from `crate::style` through the `From` at the border.
    terminal.backend().assert_buffer(&expected([
      Span::raw(" "),
      Span::styled("####", green()),
      Span::styled(".....", gray()),
    ]));
  }

  #[test]
  fn width_caps_the_row_and_leaves_the_rest_alone() {
    let job = Progress::new()
      .style(ProgressStyle {
        filled_symbol: "#",
        unfilled_symbol: ".",
        ..ProgressStyle::default()
      })
      .width(6);
    job.set(0.5);
    let terminal = draw(job);

    // Six columns used, the remaining four left untouched.
    terminal.backend().assert_buffer(&expected([
      Span::raw(" "),
      Span::styled("##", green()),
      Span::styled("...", gray()),
      Span::raw("    "),
    ]));
  }

  #[test]
  fn reporting_progress_moves_the_spinner() {
    let job = Progress::new();
    let start = job.spinner.get();

    job.set_label("resolving");
    job.set(0.1);
    job.tick();

    assert_eq!(job.spinner.get(), start + 3);
  }
}
