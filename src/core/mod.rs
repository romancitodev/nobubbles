mod command;
pub use command::{Batch, Command};
mod program;
pub use program::*;

use std::{fmt::Display, marker::PhantomData};

use crossterm::event::Event;

/// Subscriptions for external events (timers, websockets, etc).
/// Not implemented yet.
pub struct Subscription<Message>(PhantomData<Message>);

impl<Message> Subscription<Message> {
    /// No subscription.
    #[must_use]
    pub fn none() -> Self {
        Subscription(PhantomData)
    }
}

/// Your application state and logic.
///
/// Implement this trait to define how your app responds to events:
/// - `event()` maps terminal events to messages
/// - `update()` processes messages and changes state
/// - `view()` renders state to the screen
pub trait Model: Sized {
    /// Message type for your app's actions.
    type Message: Clone + Send;

    /// View output type (usually String).
    type Output: Display;

    /// Convert a terminal event to a message. Return None to ignore.
    fn event(&self, ev: Event) -> Option<Self::Message>;

    /// Process a message and update state. Return a Command if needed.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message>;

    /// Render the current state.
    fn view(&self) -> Self::Output;

    /// Subscribe to external events
    fn subscription(&mut self) -> Subscription<Self::Message> {
        Subscription::none()
    }
}
