//! A commit wizard that runs git for real and waits for it.
//!
//! Every step is a child process. The UI stays alive while it runs, whatever it prints lands
//! on the task's line, and the exit status decides what the transcript says. The slow step is
//! a real `sleep` process rather than a `thread::sleep`: waiting on something this program
//! does not drive is the whole point.
//!
//! Not called `git.rs`, and that matters: an example compiles to `target/debug/examples/<name>`,
//! and Windows resolves a bare program name against the calling executable's own directory
//! first. `Command::new("git")` from a `git.exe` is a fork bomb that takes the machine with it.
//!
//! Safe to run anywhere — the staging and the commit are `--dry-run`, so nothing is written.
//! That is also why untracked files are left out: `--dry-run` stages nothing, and git will not
//! commit a path it has never seen. Drop the two flags in `stage` and `commit` and it is a
//! commit wizard.

use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{Sender, channel};
use std::thread;

use eyre::{Result, WrapErr};
use nobubbles::inline::{self, Report};
use nobubbles::rimel::{self, Block, palette};

/// Conventional type, gitmoji, and the aside the picker shows for it.
const TYPES: [(&str, &str, &str); 6] = [
  ("feat", "✨", "a new capability"),
  ("fix", "🐛", "a bug someone could hit"),
  ("docs", "📝", "prose only"),
  ("refactor", "♻️", "same behaviour, better shape"),
  ("test", "✅", "tests only"),
  ("chore", "🔧", "tooling, deps, config"),
];

const ACCENT: nobubbles::rimel::Color = palette::MAUVE;

/// Runs a command to the end, handing every line it prints to `say`.
///
/// Both pipes are drained on their own threads. Reading one only after the other has finished
/// is how a pipe fills up and both sides stop for good, and git talks on both.
fn run(
  program: &str,
  args: impl IntoIterator<Item: AsRef<OsStr>>,
  say: impl Fn(&str),
) -> Result<(ExitStatus, Vec<String>)> {
  let mut child = Command::new(program)
    .args(args)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .wrap_err_with(|| format!("could not start {program}"))?;

  let (tx, rx) = channel();
  pipe(child.stdout.take(), &tx);
  pipe(child.stderr.take(), &tx);
  drop(tx); // the readers hold the last senders, so `rx` ends when both pipes close

  let mut said = Vec::new();
  for line in rx {
    say(&line);
    said.push(line);
  }

  Ok((child.wait()?, said))
}

fn pipe(stream: Option<impl Read + Send + 'static>, tx: &Sender<String>) {
  let Some(stream) = stream else { return };
  let tx = tx.clone();

  thread::spawn(move || {
    for line in BufReader::new(stream).lines().map_while(|line| line.ok()) {
      let _ = tx.send(line);
    }
  });
}

/// The same, with no UI around it, for what runs before the session opens.
fn quietly(args: impl IntoIterator<Item: AsRef<OsStr>>) -> Vec<String> {
  run("git", args, |_| {}).map(|(_, said)| said).unwrap_or_default()
}

/// A real process that takes its time and does nothing. On a real commit this is gpg.
fn nap() -> (&'static str, Vec<String>) {
  if cfg!(windows) {
    (
      "powershell",
      vec![
        "-NoProfile".to_owned(),
        "-Command".to_owned(),
        "Start-Sleep -Milliseconds 1200".to_owned(),
      ],
    )
  } else {
    ("sleep", vec!["1.2".to_owned()])
  }
}

/// `XY path`, porcelain v1. A rename reads `old -> new`, and it is the new name that stages.
fn path_of(line: &str) -> String {
  let path = line.get(3..).unwrap_or(line).trim();
  path
    .rsplit(" -> ")
    .next()
    .unwrap_or(path)
    .trim_matches('"')
    .to_owned()
}

/// `--dry-run` stages nothing, so a path git has never seen is not one it can be asked to
/// commit — it answers `pathspec did not match`.
fn untracked(line: &str) -> bool {
  line.starts_with("??")
}

fn report_status(what: &str, status: ExitStatus) {
  if !status.success() {
    inline::log::error(format!("{what} exited with {status}"));
  }
}

/// Printed before the session opens, where a bare `\n` still returns the carriage.
fn banner(branch: &str, changes: usize) {
  let badge = rimel::text("commit").bg(ACCENT).fg(palette::BASE).bold().px(1);
  let aside = rimel::text(format!("  {branch} · {changes} changes")).dim();

  println!();
  println!("{}", rimel::row([badge, aside]));
  println!("{}", rimel::separator(34).dim());
}

/// What the commit is going to be, boxed, before anyone is asked to confirm it.
fn summary(message: &str, branch: &str, stat: &str) -> Block {
  let body = rimel::col([
    rimel::text(message).bold(),
    rimel::text(""),
    rimel::text(format!("{stat}  ·  onto {branch}")).dim(),
  ]);

  body.px(1).rounded().border_color(ACCENT)
}

fn stage(paths: &[String]) -> Result<()> {
  let mut args = vec!["add".to_owned(), "--dry-run".to_owned(), "--".to_owned()];
  args.extend(paths.iter().cloned());

  // The outer `?` is the prompt's — Ctrl+C. The inner one is git's.
  let (status, _) = inline::task("Staging", move |report: &Report| {
    run("git", args, |line| report.say(line))
  })??;

  report_status("git add", status);
  Ok(())
}

fn commit(message: &str, paths: &[String]) -> Result<Vec<String>> {
  let mut args = vec![
    "commit".to_owned(),
    "--dry-run".to_owned(),
    "-m".to_owned(),
    message.to_owned(),
    "--".to_owned(),
  ];
  args.extend(paths.iter().cloned());

  let (status, said) = inline::task("Committing", move |report: &Report| {
    report.say("waiting on the gpg agent");
    let (program, sleeping) = nap();
    run(program, sleeping, |_| {})?;

    report.say("git commit");
    run("git", args, |line| report.say(line))
  })??;

  report_status("git commit", status);
  Ok(said)
}

fn main() -> Result<()> {
  let branch = quietly(["rev-parse", "--abbrev-ref", "HEAD"])
    .first()
    .cloned()
    .unwrap_or_else(|| "detached".to_owned());
  let changes = quietly(["status", "--porcelain"]);

  banner(&branch, changes.len());
  let session = inline::intro("What are we committing")?;

  if changes.is_empty() {
    inline::log::warn("nothing to commit, the working tree is clean");
    inline::outro(session).with("Nothing to do");
    return Ok(());
  }

  let (tracked, new_files): (Vec<&String>, Vec<&String>) =
    changes.iter().partition(|line| !untracked(line));

  if !new_files.is_empty() {
    inline::log::warn(format!(
      "{} untracked file(s) left out: a dry run can't stage what git has never seen",
      new_files.len()
    ));
  }

  if tracked.is_empty() {
    inline::outro(session).with("Only untracked files, nothing a dry run can commit");
    return Ok(());
  }

  let picked = inline::multiselect("What goes in")
    .items(tracked.iter().map(|line| (*line).clone()))
    .max_rows(8)
    .ask()?;

  if picked.is_empty() {
    inline::outro(session).with("Nothing picked, nothing committed");
    return Ok(());
  }

  let paths: Vec<String> = picked.iter().map(|&at| path_of(tracked[at])).collect();

  // `--shortstat` answers in one line, which is exactly what the summary box has room for.
  let mut diff = vec!["diff".to_owned(), "--shortstat".to_owned(), "--".to_owned()];
  diff.extend(paths.iter().cloned());

  let (_, stat) = inline::task("Measuring", move |report: &Report| {
    run("git", diff, |line| report.say(line.trim()))
  })??;
  let stat = stat
    .first()
    .map_or_else(|| format!("{} files", paths.len()), |line| line.trim().to_owned());

  let mut kinds = inline::select("Type").items(TYPES.map(|(name, emoji, _)| {
    format!("{name}  {emoji}")
  }));
  for (at, (.., note)) in TYPES.iter().enumerate() {
    kinds = kinds.note(at, format!("({note})"));
  }
  let kind = TYPES[kinds.strict().ask()?];

  let scope = inline::input("Scope").placeholder("e.g. auth, api").ask()?;
  let summary_line = inline::input("Summary")
    .placeholder("short, imperative: add x, fix y")
    .validate(|value| {
      if value.trim().is_empty() {
        Err("a commit needs a subject".into())
      } else {
        Ok(())
      }
    })
    .ask()?;

  let scope = scope.trim();
  let head = if scope.is_empty() {
    kind.0.to_owned()
  } else {
    format!("{}({scope})", kind.0)
  };
  let message = format!("{head}: {} {}", kind.1, summary_line.trim());

  inline::log::block(&summary(&message, &branch, &stat));

  if !inline::confirm("Commit it?").ask()? {
    inline::outro(session).with("Nothing committed");
    return Ok(());
  }

  stage(&paths)?;
  let said = commit(&message, &paths)?;

  let last = said.last().cloned().unwrap_or_default();
  inline::outro(session).with(format!("{message}\n\n{last}"));
  Ok(())
}
