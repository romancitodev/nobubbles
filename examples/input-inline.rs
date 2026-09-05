use eyre::Result;
use nobubbles::inline;

const LANGS: [&str; 3] = ["Rust", "Python", "JavaScript"];

fn main() -> Result<()> {
  let session = inline::intro("XDev Profile Preferences")?;

  let user = inline::input("Write your own username: @")?;
  let lang = inline::select("Preferred lang: ", LANGS)?;

  inline::outro(session).with(format!(
    "Hi @{user}, you choose {} (index {lang}).",
    LANGS[lang]
  ));
  Ok(())
}
