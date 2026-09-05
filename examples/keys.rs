use nobubbles::app::Inline;
use nobubbles::components::text::Text;
use nobubbles::signals::{quit, signal};

fn main() -> eyre::Result<()> {
  let last = signal(String::from("apretá teclas, q para salir"));
  Inline::run(30, |cx| {
    if let Some(k) = cx.key() {
      if k.code == crossterm::event::KeyCode::Char('q') {
        quit();
      }
      last.set(format!("{:?} + {:?}", k.code, k.modifiers));
    }
    cx.render(Text::new(last.get()));
  })
}
