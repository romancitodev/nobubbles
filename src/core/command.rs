use std::{future::Future, pin::Pin};

type FutCommand<Message> = Pin<Box<dyn Future<Output = Message> + Send>>;

/// Return value from `update()` to request side effects.
///
/// Most updates just change state and return `Command::none()`.
/// Return `Command::quit()` when you want to exit the app.
pub enum Command<Message: Clone + Send> {
    /// Just update state, nothing else.
    None,
    /// Run async work. Not implemented yet.
    #[allow(dead_code)]
    Perform(FutCommand<Message>),
    /// Exit the application.
    Quit,
    /// Run multiple commands. Not implemented yet.
    #[allow(dead_code)]
    Batch(Vec<Command<Message>>),
}

impl<Message: Clone + Send> Command<Message> {
    /// No side effects.
    #[must_use]
    pub fn none() -> Self {
        Command::None
    }

    /// Exit the app cleanly.
    #[must_use]
    pub fn quit() -> Self {
        Command::Quit
    }

    /// Run an async operation. Not implemented.
    #[allow(dead_code)]
    #[must_use]
    pub fn perform(function: FutCommand<Message>) -> Self {
        Command::Perform(function)
    }

    /// Used internally to check for quit signal.
    pub(crate) fn is_quit(&self) -> bool {
        matches!(self, Command::Quit)
    }
}

/// Run multiple commands at once. Not implemented.
#[allow(dead_code)]
pub struct Batch<Message: Clone + Send> {
    commands: Vec<Command<Message>>,
}

impl<Message: Clone + Send> Batch<Message> {
    #[allow(dead_code)]
    pub fn new(commands: Vec<Command<Message>>) -> Self {
        Batch { commands }
    }
}
