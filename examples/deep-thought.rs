//! deep-thought — most of what the crate does, in one sitting.
//!
//! `cargo run --example deep-thought --features gradient`
//!
//! Every prompt but the password one, log lines, a background task, a live view driven from a
//! worker thread, and rímel doing the parts a widget can't: the badge, the boxes, the block
//! art, and the titles when rainbow mode is on.
//!
//! Rainbow mode is a question the CLI asks. Say yes and every prompt title gets a pastel ramp
//! — a title is a rímel block, so it can carry its own colours. Say no and it is catppuccin
//! all the way down. Either way the ramp on the bar and on the answer stays: a CLI where
//! everything is a rainbow is a CLI where nothing is.

use std::thread;
use std::time::Duration;

use eyre::Result;
use nobubbles::app::Inline;
use nobubbles::column;
use nobubbles::components::prompt::{Prompt, PromptState};
use nobubbles::components::{Column, Render, progress::Progress, text::Text};
use nobubbles::effects::{self, Emitter};
use nobubbles::inline;
use nobubbles::rimel::{self, Align, Block, Ramp, colorgrad, palette};
use nobubbles::signals::quit;
use nobubbles::style::Style;

/// Columns the thinking bar takes.
const BAR: u16 = 34;
/// Ticks per chore, so the bar moves instead of jumping.
const TICKS: u32 = 6;
/// Ticks between one pro tip and the next.
const TIP_EVERY: usize = 5;
/// How fast the ramps drift, in turns of the colour wheel per second. Around 0.1 crawls, 1.0
/// is about as fast as a terminal reads without strobing.
const DRIFT: f32 = 0.45;

const CHORES: [&str; 7] = [
  "consulting the improbability drive",
  "normalising vogon poetry",
  "reticulating splines",
  "asking the mice",
  "dividing by the speed of plot",
  "carrying the two",
  "calculating the mean of life",
];

/// Kept under 68 columns: nothing here wraps yet, it clips.
const TIPS: [&str; 14] = [
  "two hard problems: cache invalidation, naming things, off-by-one",
  "weeks of coding can save you hours of planning",
  "deleted code is debugged code",
  "it works on my machine, so we are shipping my machine",
  "a TODO is a promise you make to a stranger who is also you",
  "the best error message is the one that never shows up",
  "any sufficiently advanced bug is indistinguishable from a feature",
  "99 bugs in the code, patch one, 127 bugs in the code",
  "naming things is easy; renaming them is the hard part",
  "if debugging removes bugs, programming is putting them in",
  "the borrow checker was never the problem",
  "premature optimisation is the root of most of my commits",
  "comments say why; the code already said what",
  "don't panic — it is also just good advice",
];

const QUESTIONS: [(&str, &str); 4] = [
  ("the meaning of life", "(takes a while)"),
  ("the mean of life", "(arithmetic, mostly)"),
  ("the median of life", "(nobody has asked for this)"),
  ("what the question was", "(out of scope, sorry)"),
];

const SUBSYSTEMS: [(&str, &str); 6] = [
  ("babel fish", "(translates everything, ruins wars)"),
  ("improbability drive", "(may produce a whale)"),
  ("vogon poetry filter", "(recommended)"),
  ("towel warmer", ""),
  ("bistromathics", "(non-linear, non-euclidean)"),
  ("somebody else's problem field", "(you cannot see this one)"),
];

/// Block art, because the answer deserves it.
const FORTY_TWO: &str = "\
█   █  █████
█   █      █
█████   ███
    █  █
    █  █████";

/// How this run is painted.
#[derive(Clone, Copy)]
struct Look {
  rainbow: bool,
}

impl Look {
  /// A prompt title. Under rainbow mode it is a pastel ramp that keeps drifting.
  ///
  /// A title is a rímel block, so this is the whole trick — no theme, no global, just a
  /// different block handed to the same builder. An animated one asks for its own frames, so
  /// the prompt repaints at the frame rate for as long as it is open: pretty is not free.
  fn title(self, text: &str) -> Block {
    let block = rimel::text(text);
    if self.rainbow {
      block.animate(Ramp::pastel(), DRIFT)
    } else {
      block
    }
  }
}

/// What the thinking thread has to say. The protocol belongs to the app, not the framework.
enum Step {
  Chore(&'static str),
  Tip(&'static str),
  Overall(f32),
  Subsystem { at: usize, ratio: f32 },
  Answer(u32),
}

/// Seven and a half million years, compressed.
fn think(subsystems: usize, deep: bool, sender: &Emitter<Step>) {
  let beat = Duration::from_millis(if deep { 80 } else { 40 });
  let total = (CHORES.len() as f32) * (TICKS as f32);
  let mut tips = TIPS.iter().cycle();

  for (chore, name) in CHORES.iter().enumerate() {
    sender.send(Step::Chore(name));

    for tick in 1..=TICKS {
      thread::sleep(beat);

      let step = chore * TICKS as usize + tick as usize;
      let done = step as f32 / total;
      sender.send(Step::Overall(done));

      if step.is_multiple_of(TIP_EVERY)
        && let Some(tip) = tips.next()
      {
        sender.send(Step::Tip(tip));
      }

      // Each subsystem runs a little behind the one before it, so the rows don't move in
      // lockstep and the board reads as several things working.
      for at in 0..subsystems {
        let lag = 1.0 + at as f32 * 0.18;
        sender.send(Step::Subsystem {
          at,
          ratio: (done * lag).min(1.0),
        });
      }
    }
  }

  sender.send(Step::Answer(42));
}

/// A ramp that leaves and comes back, so a pulse has no seam where it wraps.
///
/// `Ramp::new` takes any colorgrad gradient, and norimel re-exports colorgrad for exactly
/// this: the presets cover the wheel, anything else is three lines of builder.
fn breathe() -> Ramp {
  Ramp::new(
    colorgrad::GradientBuilder::new()
      .html_colors(&["#585b70", "#cba6f7", "#585b70"])
      .build::<colorgrad::LinearGradient>()
      .expect("three colours it can parse"),
  )
}

/// The bar that is actually thinking. The ramp is laid over the whole width and then cut to
/// what is done, so the colours stay put instead of stretching as it fills.
fn bar(ratio: f32) -> Block {
  let filled = (f32::from(BAR) * ratio).round().clamp(0.0, f32::from(BAR)) as u16;

  rimel::row([
    rimel::text("█".repeat(usize::from(BAR)))
      .animate(Ramp::pastel(), DRIFT * 1.5)
      .w(filled),
    rimel::text("░".repeat(usize::from(BAR - filled))).fg(palette::SURFACE1),
    rimel::text(format!("  {:>3.0}%", ratio * 100.0)).fg(palette::OVERLAY1),
  ])
}

/// The board while it thinks: the chore, the bar, a row per subsystem, a count, and the tip.
fn board(look: Look, chore: Progress, ratio: f32, rows: &[Progress], tip: &str) -> impl Render {
  let done = rows.iter().filter(|row| row.is_done()).count();
  let count = Text::new(format!("({done}/{} subsystems settled)", rows.len()))
    .style(Style::new().fg(palette::OVERLAY0));

  // The other kind of movement: not a ramp sliding across the text, but the whole line
  // breathing on one colour at a time.
  let advice = rimel::text(format!("💡 {tip}")).italic().pulse(breathe(), 0.35);

  let stack: Column = rows.iter().copied().collect();
  let body = column![chore, bar(ratio), stack, count, advice];

  Prompt::new(PromptState::Active, look.title("Thinking"), body)
}

fn compute(look: Look, picked: &[usize], deep: bool) -> Result<u32> {
  let chore = Progress::new();
  // Every label padded to the widest, or each bar starts wherever its own name ended.
  let widest = picked
    .iter()
    .map(|&at| rimel::width_of(SUBSYSTEMS[at].0))
    .max()
    .unwrap_or(0);
  let rows: Vec<Progress> = picked
    .iter()
    .map(|&at| {
      Progress::with(SUBSYSTEMS[at].0)
        .label_width(widest)
        .width(widest + 1 + BAR)
    })
    .collect();

  let inbox = effects::inbox::<Step>();
  let subsystems = rows.len();
  inbox.spawn(move |sender| think(subsystems, deep, &sender));

  let mut ratio = 0.0f32;
  let mut answer = 0;
  let mut tip = "warming up the improbability field";

  Inline::run(30, |cx| {
    // Draining is what keeps the loop awake, so the ramp drifts without a signal of its own.
    let thinking = inbox.drain(|step| match step {
      Step::Chore(name) => chore.set_label(name),
      Step::Tip(advice) => tip = advice,
      Step::Overall(value) => ratio = value,
      Step::Subsystem { at, ratio } => rows[at].set(ratio),
      Step::Answer(value) => answer = value,
    });

    if thinking {
      // The chore is alive even on the frames where the bar is the one with news, and a
      // spinner that only moves when its own label changes reads as frozen.
      chore.tick();
      cx.render(board(look, chore, ratio, &rows, tip));
    } else {
      let done =
        Text::new("7½ million years, give or take").style(Style::new().fg(palette::OVERLAY0));
      cx.render(Prompt::new(
        PromptState::Submitted,
        look.title("Thinking"),
        done,
      ));
      quit();
    }
  })?;

  Ok(answer)
}

/// Marvin, being Marvin.
fn quote() -> Block {
  rimel::col_items(
    Align::Left,
    [
      rimel::text("\"I think you ought to know").italic(),
      rimel::text(" I'm feeling very depressed.\"").italic(),
      rimel::text(""),
      rimel::text("— Marvin, the Paranoid Android")
        .fg(palette::OVERLAY0)
        .right(),
    ],
  )
  .px(2)
  .border_color(palette::SURFACE2)
}

/// The answer, boxed, with the only other ramp in the program.
fn verdict(answer: u32, question: &str) -> Block {
  rimel::col_items(
    Align::Center,
    [
      rimel::text(FORTY_TWO).gradient(Ramp::pastel()),
      rimel::text(""),
      rimel::text(format!("{question} = {answer}")).fg(palette::SUBTEXT0),
      rimel::text("(checked twice)")
        .fg(palette::OVERLAY0)
        .italic(),
    ],
  )
  .px(3)
  .py(1)
  .rounded()
  .border_color(palette::MAUVE)
}

fn banner() {
  let badge = rimel::text("deep thought")
    .bg(palette::MAUVE)
    .fg(palette::BASE)
    .bold()
    .px(1);

  println!();
  println!(
    "{}",
    rimel::row([
      badge,
      rimel::text("  v42.0.0 · mostly harmless").fg(palette::OVERLAY1),
    ])
  );
  println!("{}", rimel::separator(48).fg(palette::SURFACE1));
  println!();
}

fn main() -> Result<()> {
  banner();
  let session = inline::intro("Deep Thought")?;

  // The one prompt that can't be rainbow: it is the one asking.
  let look = Look {
    rainbow: inline::confirm("Rainbow mode?").ask()?,
  };
  if look.rainbow {
    inline::log::info("titles will be pastel from here on. you asked for this");
  }

  inline::task(look.title("Booting"), |report| {
    report.say("warming 7½ million years of compute");
    thread::sleep(Duration::from_millis(400));
    report.say("towel: present and correct");
    thread::sleep(Duration::from_millis(320));
    report.say("engaging the somebody-else's-problem field");
    thread::sleep(Duration::from_millis(320));
  })?;

  let mut questions =
    inline::select(look.title("What should I compute?")).items(QUESTIONS.map(|(q, _)| q));
  for (at, (_, note)) in QUESTIONS.iter().enumerate() {
    questions = questions.note(at, *note);
  }
  let picked_question = questions.strict().ask()?;
  if picked_question == 3 {
    inline::log::warn("that one takes another computer, and a planet to run it on");
  }
  let question = QUESTIONS[picked_question].0;

  let deep = inline::select_key(look.title("How hard should I think?"))
    .items([
      ('q', "quick, and probably wrong"),
      ('n', "normal"),
      ('d', "deep, the way it was meant"),
    ])
    .ask()?
    == 2;

  let mut picker = inline::multiselect(look.title("Which subsystems come along"))
    .items(SUBSYSTEMS.map(|(name, _)| name))
    .max_rows(6);
  for (at, (_, note)) in SUBSYSTEMS.iter().enumerate() {
    if !note.is_empty() {
      picker = picker.note(at, *note);
    }
  }
  let picked = picker.ask()?;

  if picked.is_empty() {
    inline::log::warn("no subsystems, so the answer will be mostly vibes");
  }

  let name = inline::input(look.title("Name this instance"))
    .validate(|value| match value.trim() {
      "" => Err("even a computer gets a name".into()),
      "42" => Err("that's the answer, not the question".into()),
      "Marvin" => Err("Marvin already knows, and he isn't telling".into()),
      _ => Ok(()),
    })
    .ask()?;

  inline::log::step(format!("{name} will compute {question}"));
  inline::log::block(&quote());

  if !inline::confirm(look.title("Start the long think?")).ask()? {
    inline::outro(session).with("Some other aeon, then");
    return Ok(());
  }

  let answer = compute(look, &picked, deep)?;

  inline::log::success("computation settled, universe unchanged");
  inline::log::block(&verdict(answer, question));

  let kept: Vec<&str> = picked.iter().map(|&at| SUBSYSTEMS[at].0).collect();
  let mut sign_off = vec![format!("{name} is done thinking.")];
  if !kept.is_empty() {
    sign_off.push(String::new());
    sign_off.push(format!("Subsystems: {}", kept.join(", ")));
  }
  sign_off.push(String::new());
  sign_off.push("Now go and work out what the question was.".to_owned());
  sign_off.push("So long, and thanks for all the fish.".to_owned());

  inline::outro(session).with(sign_off.join("\n"));
  Ok(())
}
