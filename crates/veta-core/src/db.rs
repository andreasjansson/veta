use crate::{CreateNote, Error, Note, NoteQuery, TagCount, UpdateNote};

/// Database abstraction that works for both SQLite and D1.
///
/// Uses `async_trait` with `?Send` bound for WASM compatibility.
/// The `?Send` is critical - WASM is single-threaded and JS values aren't Send.
#[async_trait::async_trait(?Send)]
pub trait Database {
    /// Add a new note and return its ID.
    async fn add_note(&self, note: CreateNote) -> Result<i64, Error>;

    /// Get a note by ID.
    async fn get_note(&self, id: i64) -> Result<Option<Note>, Error>;

    /// List notes matching the query.
    async fn list_notes(&self, query: NoteQuery) -> Result<Vec<Note>, Error>;

    /// Update an existing note.
    async fn update_note(&self, id: i64, update: UpdateNote) -> Result<bool, Error>;

    /// Delete a note by ID. Returns true if deleted, false if not found.
    async fn delete_note(&self, id: i64) -> Result<bool, Error>;

    /// List all tags with their note counts.
    async fn list_tags(&self) -> Result<Vec<TagCount>, Error>;

    /// Search notes by pattern (regex) in title and body.
    async fn grep(
        &self,
        pattern: &str,
        tags: Option<&[String]>,
        case_sensitive: bool,
    ) -> Result<Vec<Note>, Error>;
}
