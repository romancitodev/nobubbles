use crossterm::event::KeyCode;
use eyre::Result;
use nobubbles::app::Inline;
use nobubbles::column;
use nobubbles::components::input::Input;
use nobubbles::components::{Component, Render};
use nobubbles::signals::{Signal, quit, signal};
use ratatui::widgets::Paragraph;

struct Form {
  user: Input,
  submitted: Signal<bool>,
}

impl Component for Form {
  fn view(&self) -> impl Render {
    if self.submitted.get() {
      column![Paragraph::new(format!(
        "¿Este es tu usuario? {}",
        self.user.value()
      ))]
    } else {
      column![Paragraph::new("Ingrese su nombre de usuario: "), self.user]
    }
  }
}

fn main() -> Result<()> {
  let form = Form {
    user: Input::new(""),
    submitted: signal(false),
  };

  Inline::run(30, |cx| {
    if let Some(key) = cx.key() {
      match key.code {
        KeyCode::Enter if form.submitted.get() => quit(),
        KeyCode::Enter => form.submitted.set(true),
        _ if !form.submitted.get() => {
          form.user.on_key(key);
        }
        _ => {}
      }
    }

    cx.render(&form);
  })
}
