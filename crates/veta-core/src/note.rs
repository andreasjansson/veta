use serde::{Deserialize, Serialize};

/// A full note with all fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    /// References to external resources (source code paths, URLs, documentation links, etc.)
    #[serde(default)]
    pub references: Vec<String>,
    pub updated_at: String,
}

/// A summary of a note for listing (truncated body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: i64,
    pub title: String,
    pub body_preview: String,
    pub tags: Vec<String>,
    pub updated_at: String,
}

/// Tag with note count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCount {
    pub name: String,
    pub count: i64,
}

/// Query parameters for listing notes.
#[derive(Debug, Default, Clone)]
pub struct NoteQuery {
    pub tags: Option<Vec<String>>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
}

/// Parameters for creating a new note.
#[derive(Debug, Clone)]
pub struct CreateNote {
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
}

/// Parameters for updating an existing note.
#[derive(Debug, Default, Clone)]
pub struct UpdateNote {
    pub title: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
}

impl Note {
    /// Convert to summary with truncated body.
    pub fn to_summary(&self, max_len: usize) -> NoteSummary {
        let body_preview = if self.body.len() > max_len {
            format!("{}...", &self.body[..max_len])
        } else {
            self.body.clone()
        };
        NoteSummary {
            id: self.id,
            title: self.title.clone(),
            body_preview,
            tags: self.tags.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}
