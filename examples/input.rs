use crossterm::event::KeyCode;
use eyre::Result;
use nobubbles::app::Inline;
use nobubbles::column;
use nobubbles::components::input::Input;
use nobubbles::components::select::Select;
use nobubbles::components::text::Text;
use nobubbles::components::{Component, Render};
use nobubbles::signals::{Signal, quit, signal};

struct Form {
  user: Input,
  submitted: Signal<bool>,
  prefferred_language: Select,
}

impl Component for Form {
  fn view(&self) -> impl Render {
    if self.submitted.get() {
      column![Text::new(format!(
        "¿Este es tu usuario? {} — lenguaje: {}",
        self.user.value(),
        self.prefferred_language.value().unwrap_or_default()
      ))]
    } else {
      column![
        Text::new("Ingrese su nombre de usuario: "),
        self.user,
        Text::new("Lenguaje preferido:"),
        self.prefferred_language,
      ]
    }
  }
}

fn main() -> Result<()> {
  let form = Form {
    user: Input::new(),
    submitted: signal(false),
    prefferred_language: Select::new(["Rust", "Python", "JavaScript"].into_iter()),
  };

  Inline::run(30, |cx| {
    if let Some(key) = cx.key() {
      match key.code {
        KeyCode::Enter if form.submitted.get() => quit(),
        KeyCode::Enter => form.submitted.set(true),
        _ if !form.submitted.get() => {
          let _ = form.user.on_key(key) || form.prefferred_language.on_key(key);
        }
        _ => {}
      }
    }

    cx.render(form.view());
  })
}
