//! Veta core library - shared types, traits, and business logic.
//!
//! This crate contains no I/O and can be compiled for any target.

mod dateparse;
mod db;
mod error;
pub mod migrations;
mod note;
mod service;

pub use dateparse::parse_human_date;
pub use db::Database;
pub use error::Error;
pub use migrations::{get_pending_migrations, Migration, MIGRATIONS, SCHEMA_VERSION};
pub use note::{CreateNote, Note, NoteQuery, NoteSummary, TagCount, UpdateNote};
pub use service::VetaService;
