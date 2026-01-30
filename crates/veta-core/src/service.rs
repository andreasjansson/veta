use crate::{CreateNote, Database, Error, Note, NoteQuery, NoteSummary, TagCount, UpdateNote};

/// The main service that contains all business logic.
/// Generic over the database implementation.
pub struct VetaService<D: Database> {
    db: D,
}

impl<D: Database> VetaService<D> {
    pub fn new(db: D) -> Self {
        Self { db }
    }

    /// Add a new note.
    pub async fn add_note(
        &self,
        title: String,
        body: String,
        tags: Vec<String>,
        references: Vec<String>,
    ) -> Result<i64, Error> {
        // Validation
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err(Error::Validation("title cannot be empty".into()));
        }

        // Normalize tags: lowercase, trim, deduplicate, remove empty
        let mut tags: Vec<String> = tags
            .into_iter()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        tags.sort();
        tags.dedup();

        // Normalize references: trim, deduplicate, remove empty
        let mut references: Vec<String> = references
            .into_iter()
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .collect();
        references.dedup();

        self.db
            .add_note(CreateNote {
                title,
                body,
                tags,
                references,
            })
            .await
    }

    /// Get a note by ID.
    pub async fn get_note(&self, id: i64) -> Result<Option<Note>, Error> {
        self.db.get_note(id).await
    }

    /// List notes with optional filters.
    pub async fn list_notes(&self, query: NoteQuery) -> Result<Vec<NoteSummary>, Error> {
        // Apply default limit if not specified (0 means no limit)
        let query = NoteQuery {
            limit: match query.limit {
                Some(0) => None,
                Some(n) => Some(n),
                None => Some(100),
            },
            ..query
        };
        let notes = self.db.list_notes(query).await?;
        Ok(notes.into_iter().map(|n| n.to_summary(140)).collect())
    }

    /// Count notes matching the query (ignores limit).
    pub async fn count_notes(&self, query: NoteQuery) -> Result<i64, Error> {
        self.db.count_notes(query).await
    }

    /// Update an existing note.
    pub async fn update_note(&self, id: i64, update: UpdateNote) -> Result<bool, Error> {
        // Validate title if provided
        if let Some(ref title) = update.title {
            if title.trim().is_empty() {
                return Err(Error::Validation("title cannot be empty".into()));
            }
        }

        // Normalize tags if provided
        let update = UpdateNote {
            title: update.title.map(|t| t.trim().to_string()),
            body: update.body,
            tags: update.tags.map(|tags| {
                let mut tags: Vec<String> = tags
                    .into_iter()
                    .map(|t| t.trim().to_lowercase())
                    .filter(|t| !t.is_empty())
                    .collect();
                tags.sort();
                tags.dedup();
                tags
            }),
            references: update.references.map(|refs| {
                let mut refs: Vec<String> = refs
                    .into_iter()
                    .map(|r| r.trim().to_string())
                    .filter(|r| !r.is_empty())
                    .collect();
                refs.dedup();
                refs
            }),
        };

        self.db.update_note(id, update).await
    }

    /// Delete a note by ID.
    pub async fn delete_note(&self, id: i64) -> Result<bool, Error> {
        self.db.delete_note(id).await
    }

    /// List all tags with counts.
    pub async fn list_tags(&self) -> Result<Vec<TagCount>, Error> {
        self.db.list_tags().await
    }

    /// Search notes by pattern.
    pub async fn grep(
        &self,
        pattern: &str,
        tags: Option<Vec<String>>,
        case_sensitive: bool,
    ) -> Result<Vec<NoteSummary>, Error> {
        let notes = self
            .db
            .grep(pattern, tags.as_deref(), case_sensitive)
            .await?;
        Ok(notes.into_iter().map(|n| n.to_summary(140)).collect())
    }
}
