//! Colors and text attributes.
//!
//! The vocabulary lives in [`norimel`], the styling crate next door, so that a block of text
//! and a widget are painted with the same words. This module is the door to it: widgets keep
//! importing `crate::style::{Color, Style}` and never learn where it came from.

pub use norimel::style::{Color, Style, palette};
