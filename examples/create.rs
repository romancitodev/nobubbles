//! A scaffolder, in the shape of `bun create`, using every prompt the crate has.
//!
//! The questions are sequential prompts, each its own run under a single session, so raw
//! mode never drops between them and the answers stay above as a transcript. The install is
//! the other face: a live view driven by a worker thread that never touches the signals and
//! sends `Step` instead.

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
    packages: &["react", "react-dom", "vite", "typescript", "@types/react"],
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

/// The flattened options of the grouped prompt below, in the same order.
const EXTRAS: [&str; 6] = [
  "eslint",
  "prettier",
  "vitest",
  "playwright",
  "husky",
  "lint-staged",
];

#[derive(Clone, Copy)]
struct Provider {
  name: &'static str,
  /// A local model has nothing to authenticate against.
  hosted: bool,
}

const PROVIDERS: [Provider; 3] = [
  Provider {
    name: "OpenAI",
    hosted: true,
  },
  Provider {
    name: "Anthropic",
    hosted: true,
  },
  Provider {
    name: "Ollama",
    hosted: false,
  },
];

/// What the installer has to say. The protocol belongs to the app, not to the framework.
#[derive(Clone, Copy)]
enum Step {
  Phase(&'static str),
  Fetching(&'static str),
  Ratio(f32),
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
    }
  }
}

/// The worker. Resolves, installs one package at a time, links.
fn work(template: Template, sender: &Emitter<Step>) {
  sender.send(Step::Phase("Resolving dependencies"));
  thread::sleep(Duration::from_millis(700));

  sender.send(Step::Phase("Installing"));
  for name in template.packages {
    sender.send(Step::Fetching(name));
    for tick in 1..=TICKS {
      thread::sleep(Duration::from_millis(30));
      sender.send(Step::Ratio(tick as f32 / TICKS as f32));
    }
  }

  sender.send(Step::Phase("Linking"));
  thread::sleep(Duration::from_millis(420));
}

/// The install sits on the same rail as the questions, so it reads as one more step and not
/// as something that escaped the session.
fn board(phase: Progress, current: Progress, done: usize, total: usize) -> impl Render {
  let counter = Text::new(format!("({done}/{total} packages)")).style(Style::new().dim().italic());

  // The package bar only earns a row once a package started. While resolving it has no ratio
  // and no label, and drawing it anyway leaves a spinner spinning over nothing.
  let rows = if current.ratio().is_some() {
    column![phase, current, counter]
  } else {
    column![phase, counter]
  };

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
      // The phase is alive even on the frames where the package bar is the one with news,
      // and a spinner that only moves when its own label changes reads as frozen.
      phase.tick();
      cx.render(board(phase, current, done, total));
    } else {
      let done = Text::new(format!("Installed {total} packages")).style(Style::new().dim());
      cx.render(Prompt::new(PromptState::Submitted, "Installing", done));
      quit();
    }
  })
}

/// The optional half of the flow: pick a provider or don't, and only ask for a key when the
/// provider has something to authenticate against.
fn ai_provider() -> Result<Option<&'static str>> {
  let Some(at) = inline::select("AI provider")
    .items(PROVIDERS.map(|p| p.name))
    .skip("none, thanks")
    .ask()?
  else {
    inline::log::info("skipping AI setup");
    return Ok(None);
  };

  let provider = PROVIDERS[at];
  if !provider.hosted {
    inline::log::info(format!("{} runs locally, no key needed", provider.name));
    return Ok(Some(provider.name));
  }

  let title = format!("{} API key", provider.name);
  let _key = inline::password(&title)
    .validate(|key| {
      if key.len() < 8 {
        Err("that looks too short for a key".into())
      } else {
        Ok(())
      }
    })
    .ask()?;

  inline::log::success(format!("{} key written to .env", provider.name));
  Ok(Some(provider.name))
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
    .strict()
    .ask()?];

  let ai = ai_provider()?;

  let extras = inline::group_multiselect("Extras")
    .group("Quality", ["eslint", "prettier"])
    .group("Testing", ["vitest", "playwright"])
    .group("Tooling", ["husky", "lint-staged"])
    .max_rows(6)
    .ask()?;

  if inline::confirm("Initialise a git repository?").ask()? {
    inline::task("Initialising git", |report| {
      report.say("git init");
      thread::sleep(Duration::from_millis(300));
      report.say("staging the scaffold");
      thread::sleep(Duration::from_millis(400));
      report.say("initial commit");
      thread::sleep(Duration::from_millis(300));
    })?;
  }

  let how = inline::select_key("Install dependencies now?")
    .items([
      ('y', "yes, install them"),
      ('n', "no, later"),
      ('p', "print the command and stop"),
    ])
    .ask()?;

  match how {
    0 => install(template)?,
    2 => inline::log::warn(format!("run `{manager} install` when you are ready")),
    _ => {}
  }

  // The grouped prompt hands back indices over the options alone, headings not counted, so
  // the flat list stays the source of truth.
  let picked: Vec<&str> = extras.iter().map(|&i| EXTRAS[i]).collect();

  let mut summary = vec![format!("Scaffolded {name} with {}", template.name)];

  let about = about.trim();
  if !about.is_empty() {
    summary.push(String::new());
    summary.extend(about.lines().map(str::to_owned));
  }

  if let Some(provider) = ai {
    summary.push(String::new());
    summary.push(format!("AI provider: {provider}"));
  }
  if !picked.is_empty() {
    summary.push(format!("Extras: {}", picked.join(", ")));
  }

  summary.push(String::new());
  summary.push("Next steps:".to_owned());
  summary.push(format!("cd {name}"));
  if how != 0 {
    summary.push(format!("{manager} install"));
  }
  summary.push(format!("{manager} {}", template.dev));

  // The outro indents every line after the first, so these stay bare.
  inline::outro(session).with(summary.join("\n"));
  Ok(())
}
