//! Veta core library - shared types, traits, and business logic.
//!
//! This crate contains no I/O and can be compiled for any target.

mod error;
mod note;
mod db;
mod service;

pub use error::Error;
pub use note::{Note, NoteSummary, TagCount, NoteQuery, CreateNote, UpdateNote};
pub use db::Database;
pub use service::VetaService;
