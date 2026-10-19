//! Read diagram files people and agents already write — Mermaid, draw.io, DBML and Excalidraw —
//! into one graph model, and edit them back surgically: an edit rewrites as little of the
//! author's text as possible, and Schematlas's own information (links to code, notes, kinds)
//! lives inside the same file in a form the original tools ignore.
//!
//! Pure functions only: no I/O, no clock, so the crate runs natively and in WebAssembly.
pub mod annotate;
#[cfg(feature = "formats")]
pub mod dbml;
#[cfg(feature = "formats")]
pub mod drawio;
mod error;
#[cfg(feature = "formats")]
pub mod excalidraw;
#[cfg(feature = "formats")]
pub mod markdown;
#[cfg(feature = "formats")]
pub mod mermaid;
pub mod model;
#[cfg(feature = "formats")]
pub mod sketch;

pub use error::{Error, Result};
