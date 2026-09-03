use crossterm::event::{Event, KeyCode};

use crate::core::{Command, Model};

/// Text input component. Not implemented yet.
pub struct Input {
    cursor: u32,
    inner: String,
}

#[derive(Debug, Clone)]
pub enum InputMsg {
    Char(char),
    Delete,
    Submit,
}

impl Model for Input {
    type Message = InputMsg;
    type Output = String;

    fn event(&self, ev: Event) -> Option<Self::Message> {
        let Event::Key(key) = ev else {
            return None;
        };
        match key.code {
            KeyCode::Backspace => Some(InputMsg::Delete),
            KeyCode::Enter => Some(InputMsg::Submit),
            KeyCode::Char(k) => Some(InputMsg::Char(k)),
            _ => None,
        }
    }

    fn view(&self) -> Self::Output {
        self.inner.clone()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            InputMsg::Char(c) => self.inner.push(c),
            InputMsg::Delete => {
                self.inner = self
                    .inner
                    .get(0..self.inner.len().saturating_sub(1))
                    .unwrap()
                    .to_owned();
            }
            InputMsg::Submit => {}
        };
        Command::none()
    }
}
