use crossterm::event::{Event, KeyCode};
use nobubbles::components::Input;
use nobubbles::core::{Command, Model, ProgramBuilder};

#[derive(Debug, Clone)]
enum Msg {
    Next,
    Previous,
    Quit,
}

#[derive(Debug, Clone)]
enum Step {
    A,
    B,
    C,
}

impl Step {
    fn next(&self) -> Option<Step> {
        match self {
            Step::A => Some(Step::B),
            Step::B => Some(Step::C),
            Step::C => None,
        }
    }

    fn previous(&self) -> Option<Step> {
        match self {
            Step::A => None,
            Step::B => Some(Step::A),
            Step::C => Some(Step::B),
        }
    }
}

struct App {
    step: Step,
}

impl App {
    pub fn new() -> App {
        App { step: Step::A }
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
            KeyCode::Right => Some(Msg::Next),
            KeyCode::Left => Some(Msg::Previous),
            _ => None,
        }
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Msg::Next => {
                self.step = self.step.next().unwrap_or(self.step.clone());
                Command::none()
            }
            Msg::Previous => {
                self.step = self.step.previous().unwrap_or(self.step.clone());
                Command::none()
            }
            Msg::Quit => Command::quit(),
        }
    }

    fn view(&self) -> Self::Output {
        match self.step {
            Step::A => "Step A: Welcome to the wizard!\nPress Right to continue, Left to go back, 'q' to quit.".to_string(),
            Step::B => "Step B: This is the second step.\nPress Right to continue, Left to go back, 'q' to quit.".to_string(),
            Step::C => "Step C: Final step!\nPress Left to go back, 'q' to quit.".to_string(),
        }
    }
}

fn main() {
    let program = ProgramBuilder::new(App::new())
        .with_alt_screen(false)
        .build();
    program.run().unwrap();
}
