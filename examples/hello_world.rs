use crossterm::event::{Event, KeyCode, KeyEvent};
use nobubbles::core::{Command, Model, Program, ProgramBuilder};

#[derive(Debug, Clone)]
enum Msg {
    Increment,
    Decrement,
    Quit,
}

struct App {
    counter: i32,
}

impl App {
    pub fn new() -> App {
        App { counter: 0 }
    }
}

impl Model for App {
    type Message = Msg;
    type Output = String;

    fn event(&self, ev: Event) -> Option<Self::Message> {
        let Event::Key(key) = ev else {
            return None;
        };

        match key.code {
            KeyCode::Char('q') => Some(Msg::Quit),
            KeyCode::Up => Some(Msg::Increment),
            KeyCode::Down => Some(Msg::Decrement),
            _ => None,
        }
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Msg::Increment => {
                self.counter += 1;
                Command::none()
            }
            Msg::Decrement => {
                self.counter -= 1;
                Command::none()
            }
            Msg::Quit => Command::quit(),
        }
    }

    fn view(&self) -> Self::Output {
        format!(
            "Counter: {}\nPress Up/Down to change, 'q' to quit.",
            self.counter
        )
    }
}

fn main() {
    let program = ProgramBuilder::new(App::new())
        .with_alt_screen(false)
        .build();
    program.run().unwrap();
}
