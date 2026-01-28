//! D1 implementation of the Veta database trait for Cloudflare Workers.

use regex::Regex;
use serde::Deserialize;
use veta_core::{CreateNote, Database, Error, Note, NoteQuery, TagCount, UpdateNote};
use wasm_bindgen::JsValue;
use worker::d1::D1Database;

/// D1-backed database implementation.
pub struct D1DatabaseWrapper {
    db: D1Database,
}

impl D1DatabaseWrapper {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    /// Run database migrations.
    pub async fn run_migrations(&self) -> Result<(), Error> {
        // Run each statement separately since exec() has issues with multiple statements
        let statements = [
            "CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            "CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE
            )",
            "CREATE TABLE IF NOT EXISTS note_tags (
                note_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                PRIMARY KEY (note_id, tag_id)
            )",
            "CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at)",
            "CREATE INDEX IF NOT EXISTS idx_note_tags_tag_id ON note_tags(tag_id)",
            "CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name)",
        ];

        for sql in statements {
            self.db
                .prepare(sql)
                .run()
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
        }
        Ok(())
    }

    fn parse_tags(tags_str: Option<String>) -> Vec<String> {
        let mut tags: Vec<String> = tags_str
            .map(|s| {
                s.split(',')
                    .map(String::from)
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        tags.sort();
        tags
    }
}

#[derive(Deserialize)]
struct NoteIdRow {
    id: i64,
}

#[derive(Deserialize)]
struct NoteRow {
    id: i64,
    title: String,
    body: String,
    updated_at: String,
    tags: Option<String>,
}

impl NoteRow {
    fn into_note(self) -> Note {
        Note {
            id: self.id,
            title: self.title,
            body: self.body,
            updated_at: self.updated_at,
            tags: D1DatabaseWrapper::parse_tags(self.tags),
        }
    }
}

#[derive(Deserialize)]
struct TagCountRow {
    name: String,
    count: i64,
}

#[derive(Deserialize)]
struct CountRow {
    count: i64,
}

#[async_trait::async_trait(?Send)]
impl Database for D1DatabaseWrapper {
    async fn add_note(&self, note: CreateNote) -> Result<i64, Error> {
        // Insert the note
        let stmt = self
            .db
            .prepare("INSERT INTO notes (title, body) VALUES (?1, ?2) RETURNING id")
            .bind(&[
                JsValue::from_str(&note.title),
                JsValue::from_str(&note.body),
            ])
            .map_err(|e| Error::Database(e.to_string()))?;

        let result = stmt
            .first::<NoteIdRow>(None)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .ok_or_else(|| Error::Database("Failed to insert note".into()))?;

        let note_id = result.id;

        // Insert tags using batch for efficiency
        if !note.tags.is_empty() {
            let mut statements = Vec::new();

            for tag in &note.tags {
                let tag_stmt = self
                    .db
                    .prepare(
                        "INSERT INTO tags (name) VALUES (?1) ON CONFLICT (name) DO NOTHING",
                    )
                    .bind(&[JsValue::from_str(tag)])
                    .map_err(|e| Error::Database(e.to_string()))?;
                statements.push(tag_stmt);

                let link_stmt = self
                    .db
                    .prepare(
                        "INSERT INTO note_tags (note_id, tag_id) SELECT ?1, id FROM tags WHERE name = ?2",
                    )
                    .bind(&[JsValue::from_f64(note_id as f64), JsValue::from_str(tag)])
                    .map_err(|e| Error::Database(e.to_string()))?;
                statements.push(link_stmt);
            }

            self.db
                .batch(statements)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
        }

        Ok(note_id)
    }

    async fn get_note(&self, id: i64) -> Result<Option<Note>, Error> {
        let stmt = self
            .db
            .prepare(
                "SELECT n.id, n.title, n.body, n.updated_at, GROUP_CONCAT(t.name) as tags
                 FROM notes n
                 LEFT JOIN note_tags nt ON n.id = nt.note_id
                 LEFT JOIN tags t ON nt.tag_id = t.id
                 WHERE n.id = ?1
                 GROUP BY n.id",
            )
            .bind(&[JsValue::from_f64(id as f64)])
            .map_err(|e| Error::Database(e.to_string()))?;

        let row = stmt
            .first::<NoteRow>(None)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.map(|r| r.into_note()))
    }

    async fn list_notes(&self, query: NoteQuery) -> Result<Vec<Note>, Error> {
        // Build query - D1 doesn't support dynamic parameter binding well,
        // so we need to be careful here. For safety, we'll use simple queries.
        let limit = query.limit.unwrap_or(20);

        let result = if let Some(ref tags) = query.tags {
            if !tags.is_empty() {
                // Query with tag filter - use IN clause with escaped values
                let tags_list = tags
                    .iter()
                    .map(|t| format!("'{}'", t.replace('\'', "''")))
                    .collect::<Vec<_>>()
                    .join(",");

                let sql = format!(
                    "SELECT n.id, n.title, n.body, n.updated_at, GROUP_CONCAT(t.name) as tags
                     FROM notes n
                     LEFT JOIN note_tags nt ON n.id = nt.note_id
                     LEFT JOIN tags t ON nt.tag_id = t.id
                     WHERE n.id IN (
                         SELECT note_id FROM note_tags nt2
                         JOIN tags t2 ON nt2.tag_id = t2.id
                         WHERE t2.name IN ({})
                     )
                     GROUP BY n.id
                     ORDER BY n.updated_at DESC, n.id DESC
                     LIMIT {}",
                    tags_list, limit
                );

                self.db
                    .prepare(&sql)
                    .all()
                    .await
                    .map_err(|e| Error::Database(e.to_string()))?
            } else {
                self.db
                    .prepare(&format!(
                        "SELECT n.id, n.title, n.body, n.updated_at, GROUP_CONCAT(t.name) as tags
                         FROM notes n
                         LEFT JOIN note_tags nt ON n.id = nt.note_id
                         LEFT JOIN tags t ON nt.tag_id = t.id
                         GROUP BY n.id
                         ORDER BY n.updated_at DESC, n.id DESC
                         LIMIT {}",
                        limit
                    ))
                    .all()
                    .await
                    .map_err(|e| Error::Database(e.to_string()))?
            }
        } else {
            self.db
                .prepare(&format!(
                    "SELECT n.id, n.title, n.body, n.updated_at, GROUP_CONCAT(t.name) as tags
                     FROM notes n
                     LEFT JOIN note_tags nt ON n.id = nt.note_id
                     LEFT JOIN tags t ON nt.tag_id = t.id
                     GROUP BY n.id
                     ORDER BY n.updated_at DESC, n.id DESC
                     LIMIT {}",
                    limit
                ))
                .all()
                .await
                .map_err(|e| Error::Database(e.to_string()))?
        };

        let rows: Vec<NoteRow> = result.results().map_err(|e| Error::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| r.into_note()).collect())
    }

    async fn update_note(&self, id: i64, update: UpdateNote) -> Result<bool, Error> {
        // Check if note exists
        let count_stmt = self
            .db
            .prepare("SELECT COUNT(*) as count FROM notes WHERE id = ?1")
            .bind(&[JsValue::from_f64(id as f64)])
            .map_err(|e| Error::Database(e.to_string()))?;

        let exists = count_stmt
            .first::<CountRow>(None)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .map(|r| r.count > 0)
            .unwrap_or(false);

        if !exists {
            return Ok(false);
        }

        // Update title if provided
        if let Some(ref title) = update.title {
            self.db
                .prepare("UPDATE notes SET title = ?1, updated_at = datetime('now') WHERE id = ?2")
                .bind(&[JsValue::from_str(title), JsValue::from_f64(id as f64)])
                .map_err(|e| Error::Database(e.to_string()))?
                .run()
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
        }

        // Update body if provided
        if let Some(ref body) = update.body {
            self.db
                .prepare("UPDATE notes SET body = ?1, updated_at = datetime('now') WHERE id = ?2")
                .bind(&[JsValue::from_str(body), JsValue::from_f64(id as f64)])
                .map_err(|e| Error::Database(e.to_string()))?
                .run()
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
        }

        // Update tags if provided
        if let Some(ref tags) = update.tags {
            // Delete existing tags
            self.db
                .prepare("DELETE FROM note_tags WHERE note_id = ?1")
                .bind(&[JsValue::from_f64(id as f64)])
                .map_err(|e| Error::Database(e.to_string()))?
                .run()
                .await
                .map_err(|e| Error::Database(e.to_string()))?;

            // Insert new tags
            if !tags.is_empty() {
                let mut statements = Vec::new();

                for tag in tags {
                    let tag_stmt = self
                        .db
                        .prepare(
                            "INSERT INTO tags (name) VALUES (?1) ON CONFLICT (name) DO NOTHING",
                        )
                        .bind(&[JsValue::from_str(tag)])
                        .map_err(|e| Error::Database(e.to_string()))?;
                    statements.push(tag_stmt);

                    let link_stmt = self
                        .db
                        .prepare(
                            "INSERT INTO note_tags (note_id, tag_id) SELECT ?1, id FROM tags WHERE name = ?2",
                        )
                        .bind(&[JsValue::from_f64(id as f64), JsValue::from_str(tag)])
                        .map_err(|e| Error::Database(e.to_string()))?;
                    statements.push(link_stmt);
                }

                self.db
                    .batch(statements)
                    .await
                    .map_err(|e| Error::Database(e.to_string()))?;
            }

            // Update timestamp
            self.db
                .prepare("UPDATE notes SET updated_at = datetime('now') WHERE id = ?1")
                .bind(&[JsValue::from_f64(id as f64)])
                .map_err(|e| Error::Database(e.to_string()))?
                .run()
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
        }

        Ok(true)
    }

    async fn delete_note(&self, id: i64) -> Result<bool, Error> {
        // Check if note exists first
        let count_stmt = self
            .db
            .prepare("SELECT COUNT(*) as count FROM notes WHERE id = ?1")
            .bind(&[JsValue::from_f64(id as f64)])
            .map_err(|e| Error::Database(e.to_string()))?;

        let exists = count_stmt
            .first::<CountRow>(None)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .map(|r| r.count > 0)
            .unwrap_or(false);

        if !exists {
            return Ok(false);
        }

        // Delete note_tags first (foreign key)
        self.db
            .prepare("DELETE FROM note_tags WHERE note_id = ?1")
            .bind(&[JsValue::from_f64(id as f64)])
            .map_err(|e| Error::Database(e.to_string()))?
            .run()
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        // Delete note
        self.db
            .prepare("DELETE FROM notes WHERE id = ?1")
            .bind(&[JsValue::from_f64(id as f64)])
            .map_err(|e| Error::Database(e.to_string()))?
            .run()
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(true)
    }

    async fn list_tags(&self) -> Result<Vec<TagCount>, Error> {
        let result = self
            .db
            .prepare(
                "SELECT t.name, COUNT(nt.note_id) as count
                 FROM tags t
                 LEFT JOIN note_tags nt ON t.id = nt.tag_id
                 GROUP BY t.id
                 HAVING count > 0
                 ORDER BY count DESC, t.name",
            )
            .all()
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        let rows: Vec<TagCountRow> =
            result.results().map_err(|e| Error::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| TagCount {
                name: r.name,
                count: r.count,
            })
            .collect())
    }

    async fn grep(
        &self,
        pattern: &str,
        tags: Option<&[String]>,
        case_sensitive: bool,
    ) -> Result<Vec<Note>, Error> {
        // Build regex for client-side filtering
        let regex = if case_sensitive {
            Regex::new(pattern).map_err(|e| Error::Validation(format!("invalid regex: {}", e)))?
        } else {
            Regex::new(&format!("(?i){}", pattern))
                .map_err(|e| Error::Validation(format!("invalid regex: {}", e)))?
        };

        // Query all notes (with tag filter if provided)
        let result = if let Some(tag_list) = tags {
            if !tag_list.is_empty() {
                let tags_str = tag_list
                    .iter()
                    .map(|t| format!("'{}'", t.replace('\'', "''")))
                    .collect::<Vec<_>>()
                    .join(",");

                let sql = format!(
                    "SELECT n.id, n.title, n.body, n.updated_at, GROUP_CONCAT(t.name) as tags
                     FROM notes n
                     LEFT JOIN note_tags nt ON n.id = nt.note_id
                     LEFT JOIN tags t ON nt.tag_id = t.id
                     WHERE n.id IN (SELECT note_id FROM note_tags nt2 
                                    JOIN tags t2 ON nt2.tag_id = t2.id 
                                    WHERE t2.name IN ({}))
                     GROUP BY n.id
                     ORDER BY n.updated_at DESC",
                    tags_str
                );

                self.db
                    .prepare(&sql)
                    .all()
                    .await
                    .map_err(|e| Error::Database(e.to_string()))?
            } else {
                self.db
                    .prepare(
                        "SELECT n.id, n.title, n.body, n.updated_at, GROUP_CONCAT(t.name) as tags
                         FROM notes n
                         LEFT JOIN note_tags nt ON n.id = nt.note_id
                         LEFT JOIN tags t ON nt.tag_id = t.id
                         GROUP BY n.id
                         ORDER BY n.updated_at DESC",
                    )
                    .all()
                    .await
                    .map_err(|e| Error::Database(e.to_string()))?
            }
        } else {
            self.db
                .prepare(
                    "SELECT n.id, n.title, n.body, n.updated_at, GROUP_CONCAT(t.name) as tags
                     FROM notes n
                     LEFT JOIN note_tags nt ON n.id = nt.note_id
                     LEFT JOIN tags t ON nt.tag_id = t.id
                     GROUP BY n.id
                     ORDER BY n.updated_at DESC",
                )
                .all()
                .await
                .map_err(|e| Error::Database(e.to_string()))?
        };

        let rows: Vec<NoteRow> = result.results().map_err(|e| Error::Database(e.to_string()))?;

        // Filter by regex client-side
        let matching: Vec<Note> = rows
            .into_iter()
            .map(|r| r.into_note())
            .filter(|note| regex.is_match(&note.title) || regex.is_match(&note.body))
            .collect();

        Ok(matching)
    }
}
