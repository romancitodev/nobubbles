//! A scaffolder, in the shape of `bun create`: a few questions, then the install.
//!
//! It's the whole crate in one file. The questions are sequential prompts, each its own run
//! under a single session, so raw mode never drops between them and the answers stay above
//! as a transcript. The install is the other face: one live view driven by a worker thread
//! that can't touch the signals and sends `Step` instead.

use std::thread;
use std::time::Duration;

use eyre::Result;
use nobubbles::app::Inline;
use nobubbles::column;
use nobubbles::components::prompt::{Prompt, PromptState};
use nobubbles::components::{Render, progress::Progress, text::Text};
use nobubbles::effects::{self, Emitter};
use nobubbles::inline;
use nobubbles::signals::quit;
use nobubbles::style::Style;

/// Ticks per package, so a bar moves instead of jumping.
const TICKS: u32 = 10;

#[derive(Clone, Copy)]
struct Template {
  name: &'static str,
  packages: &'static [&'static str],
  dev: &'static str,
}

const TEMPLATES: [Template; 3] = [
  Template {
    name: "React",
    packages: &[
      "react",
      "react-dom",
      "vite",
      "typescript",
      "@types/react",
      "@shadcn-ui/react",
      "lucide-icons",
      "@types/nodejs",
    ],
    dev: "dev",
  },
  Template {
    name: "Svelte",
    packages: &["svelte", "vite", "typescript"],
    dev: "dev",
  },
  Template {
    name: "Bare TypeScript",
    packages: &["typescript", "@types/node"],
    dev: "start",
  },
];

const MANAGERS: [&str; 3] = ["bun", "pnpm", "npm"];
const EXTRAS: [&str; 3] = ["eslint", "prettier", "vitest"];

/// What the installer has to say. The protocol belongs to the app, not to the framework.
#[derive(Clone, Copy)]
enum Step {
  Phase(&'static str),
  Fetching(&'static str),
  Ratio(f32),
  Alive,
}

impl Step {
  /// Runs on the loop's thread, the only place the rows can be written.
  fn apply(self, phase: &Progress, current: &Progress) {
    match self {
      Self::Phase(name) => phase.set_label(name),
      Self::Fetching(name) => {
        current.set_label(name);
        current.set(0.0);
      }
      Self::Ratio(value) => current.set(value),
      // Nothing new to report, but the job is alive and the spinner should say so.
      Self::Alive => phase.tick(),
    }
  }
}

/// The worker. Resolves, installs one package at a time, links.
fn work(template: Template, sender: &Emitter<Step>) {
  sender.send(Step::Phase("Resolving dependencies"));
  for _ in 0..10 {
    thread::sleep(Duration::from_millis(70));
    sender.send(Step::Alive);
  }

  sender.send(Step::Phase("Installing"));
  for name in template.packages {
    sender.send(Step::Fetching(name));
    for tick in 1..=TICKS {
      thread::sleep(Duration::from_millis(30));
      sender.send(Step::Ratio(tick as f32 / TICKS as f32));
    }
  }

  sender.send(Step::Phase("Linking"));
  for _ in 0..6 {
    thread::sleep(Duration::from_millis(70));
    sender.send(Step::Alive);
  }
}

/// The install sits on the same rail as the questions, so it reads as one more step and
/// not as something that escaped the session.
fn board(phase: Progress, current: Progress, done: usize, total: usize) -> impl Render {
  let rows = column![
    phase,
    current,
    Text::new(format!("({done}/{total} packages)")).style(Style::new().dim().italic()),
  ];

  Prompt::new(PromptState::Active, "Installing", rows)
}

fn install(template: Template) -> Result<()> {
  let phase = Progress::new();
  let current = Progress::new().width(46);
  let total = template.packages.len();

  let inbox = effects::inbox::<Step>();
  inbox.spawn(move |sender| work(template, &sender));

  let mut done = 0;

  Inline::run(30, |cx| {
    let installing = inbox.drain(|step| {
      // The last tick of a package is its only ratio of exactly 1.0.
      if matches!(step, Step::Ratio(value) if value >= 1.0) {
        done += 1;
      }
      step.apply(&phase, &current);
    });

    if installing {
      cx.render(board(phase, current, done, total));
    } else {
      let done = Text::new(format!("Installed {total} packages")).style(Style::new().dim());
      cx.render(Prompt::new(PromptState::Submitted, "Installing", done));
      quit();
    }
  })
}

fn main() -> Result<()> {
  let session = inline::intro("create-nobubbles")?;

  let name = inline::input("Project name")
    .validate(|value| {
      if value.trim().is_empty() {
        Err("a project needs a name".into())
      } else {
        Ok(())
      }
    })
    .ask()?;

  let about = inline::input("Description").multiline().ask()?;

  let template = TEMPLATES[inline::select("Template")
    .items(TEMPLATES.map(|t| t.name))
    .strict()
    .ask()?];

  let manager = MANAGERS[inline::select("Package manager")
    .items(MANAGERS)
    .initial(0)
    .strict()
    .ask()?];

  let extras = inline::multiselect("Anything else?")
    .items(EXTRAS)
    .max_rows(2)
    .ask()?;

  let now = inline::confirm("Install dependencies now?").ask()?;

  if now {
    install(template)?;
  }

  // `multiselect` hands back indices, so `EXTRAS` stays the source of truth.
  let picked: Vec<&str> = extras.iter().map(|&i| EXTRAS[i]).collect();
  let with = if picked.is_empty() {
    template.name.to_owned()
  } else {
    format!("{} + {}", template.name, picked.join(", "))
  };

  let mut steps = vec![format!("cd {name}")];
  if !now {
    steps.push(format!("{manager} install"));
  }
  steps.push(format!("{manager} {}", template.dev));

  // The outro indents every line after the first, so these stay bare.
  let mut summary = vec![format!("✨ Scaffolded {name} with {with}")];

  let about = about.trim();
  if !about.is_empty() {
    summary.push(String::new());
    summary.extend(about.lines().map(str::to_owned));
  }

  summary.push(String::new());
  summary.push("Next steps:".to_owned());
  summary.extend(steps);

  inline::outro(session).with(summary.join("\n"));
  Ok(())
}
