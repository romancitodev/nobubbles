//! Reactive terminal UI framework. Signals instead of messages.

pub mod app;
pub mod components;
pub mod effects;
pub(crate) mod engine;
pub(crate) mod render;
pub mod signals;
pub mod style;

pub mod inline;
pub mod rimel;

/// Re-exported so `cx.effect` has something to take without a second dependency.
#[cfg(feature = "fx")]
pub use tachyonfx;
