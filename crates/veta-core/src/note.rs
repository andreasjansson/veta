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
    /// References to external resources (source code paths, URLs, documentation links, etc.)
    pub references: Vec<String>,
}

/// Parameters for updating an existing note.
#[derive(Debug, Default, Clone)]
pub struct UpdateNote {
    pub title: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    /// References to external resources (source code paths, URLs, documentation links, etc.)
    pub references: Option<Vec<String>>,
}

impl Note {
    /// Convert to summary with truncated body preview.
    pub fn to_summary(&self, max_len: usize) -> NoteSummary {
        let normalized: String = self
            .body
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect();
        let trimmed = normalized.trim();

        let mut chars = trimmed.chars();
        let preview: String = chars.by_ref().take(max_len).collect();
        let body_preview = if chars.next().is_some() {
            format!("{}...", preview)
        } else {
            preview
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

#[cfg(test)]
mod tests {
    use super::*;

    fn note(body: &str) -> Note {
        Note {
            id: 1,
            title: "t".into(),
            body: body.into(),
            tags: vec![],
            references: vec![],
            updated_at: "2026-01-01 00:00:00".into(),
        }
    }

    #[test]
    fn short_body_no_ellipsis() {
        let s = note("hello").to_summary(140);
        assert_eq!(s.body_preview, "hello");
    }

    #[test]
    fn long_ascii_truncated_with_ellipsis() {
        let body = "a".repeat(200);
        let s = note(&body).to_summary(140);
        assert_eq!(s.body_preview, format!("{}...", "a".repeat(140)));
    }

    #[test]
    fn newlines_normalized_to_spaces() {
        let s = note("foo\nbar\rbaz").to_summary(140);
        assert_eq!(s.body_preview, "foo bar baz");
    }

    #[test]
    fn truncates_on_char_boundary_with_multibyte() {
        // Regression: previously panicked with "byte index N is not a char boundary"
        // when max_len fell inside a multibyte UTF-8 character.
        let body = format!("{} ×× tail", "a".repeat(138));
        let s = note(&body).to_summary(140);
        assert!(s.body_preview.ends_with("..."));
        let preview_chars = s.body_preview.trim_end_matches("...").chars().count();
        assert_eq!(preview_chars, 140);
    }

    #[test]
    fn max_len_at_exact_multibyte_boundary() {
        // 4-byte char (emoji) right after the truncation point
        let body = format!("{}🎉 tail", "a".repeat(140));
        let s = note(&body).to_summary(140);
        assert_eq!(s.body_preview, format!("{}...", "a".repeat(140)));
    }

    #[test]
    fn empty_body() {
        let s = note("").to_summary(140);
        assert_eq!(s.body_preview, "");
    }

    #[test]
    fn body_exactly_max_len_chars() {
        let body = "a".repeat(140);
        let s = note(&body).to_summary(140);
        assert_eq!(s.body_preview, "a".repeat(140));
        assert!(!s.body_preview.ends_with("..."));
    }
}
