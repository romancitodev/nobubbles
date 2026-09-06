use eyre::Result;
use nobubbles::inline;

const LANGS: [&str; 3] = ["Rust", "Python", "JavaScript"];

fn main() -> Result<()> {
  let session = inline::intro("XDev Profile Preferences")?;

  let user = inline::input("Your username").placeholder("octocat").ask()?;
  let lang = inline::select("Preferred language")
    .items(LANGS)
    .strict()
    .ask()?;

  inline::outro(session).with(format!(
    "Hi @{user}, you choose {} (index {lang}).",
    LANGS[lang]
  ));
  Ok(())
}
