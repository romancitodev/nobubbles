//! Terminal UI framework based on The Elm Architecture.
//!
//! Build interactive terminal apps by implementing the `Model` trait:
//! - `event()` converts terminal events to messages
//! - `update()` processes messages and changes state
//! - `view()` renders state to the screen
//!
//! # Example
//!
//! ```no_run
//! use crossterm::event::{Event, KeyCode};
//! use nobubbles::core::{Command, Model, ProgramBuilder};
//!
//! #[derive(Clone)]
//! enum Msg { Quit }
//!
//! struct App;
//!
//! impl Model for App {
//!     type Message = Msg;
//!     type Output = String;
//!
//!     fn event(&self, ev: Event) -> Option<Msg> {
//!         match ev {
//!             Event::Key(key) if key.code == KeyCode::Char('q') => Some(Msg::Quit),
//!             _ => None,
//!         }
//!     }
//!
//!     fn update(&mut self, _msg: Msg) -> Command<Msg> {
//!         Command::quit()
//!     }
//!
//!     fn view(&self) -> String {
//!         "Press q to quit".to_string()
//!     }
//! }
//!
//! ProgramBuilder::new(App).build().run().unwrap();
//! ```

pub mod components;
pub mod core;
