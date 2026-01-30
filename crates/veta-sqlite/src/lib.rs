//! SQLite implementation of the Veta database trait.

use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use veta_core::{
    get_pending_migrations, CreateNote, Database, Error, Note, NoteQuery, TagCount, UpdateNote,
    SCHEMA_VERSION,
};

/// SQLite-backed database implementation.
pub struct SqliteDatabase {
    conn: Mutex<Connection>,
}

impl SqliteDatabase {
    /// Open a database at the given path and run any pending migrations.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let conn = Connection::open(path).map_err(|e| Error::Database(e.to_string()))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| Error::Database(e.to_string()))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    /// Open an in-memory database and run migrations.
    pub fn open_in_memory() -> Result<Self, Error> {
        let conn = Connection::open_in_memory().map_err(|e| Error::Database(e.to_string()))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| Error::Database(e.to_string()))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    /// Run any pending database migrations.
    fn run_migrations(&self) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();

        // Ensure _veta_meta table exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _veta_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        // Get current schema version
        let current_version: i64 = conn
            .query_row(
                "SELECT value FROM _veta_meta WHERE key = 'schema_version'",
                [],
                |row| {
                    let val: String = row.get(0)?;
                    Ok(val.parse().unwrap_or(0))
                },
            )
            .unwrap_or(0);

        // Already up to date
        if current_version >= SCHEMA_VERSION {
            return Ok(());
        }

        // Run pending migrations
        for migration in get_pending_migrations(current_version) {
            for statement in migration.statements {
                // Skip _veta_meta creation (already done above)
                if statement.contains("_veta_meta") {
                    continue;
                }
                // ALTER TABLE doesn't support IF NOT EXISTS, so ignore errors for those
                if statement.starts_with("ALTER TABLE") {
                    let _ = conn.execute(statement, []);
                } else {
                    conn.execute(statement, []).map_err(|e| {
                        Error::Database(format!("Migration {} failed: {}", migration.name, e))
                    })?;
                }
            }
        }

        // Update schema version
        conn.execute(
            "INSERT OR REPLACE INTO _veta_meta (key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

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

    fn parse_references(refs_str: Option<String>) -> Vec<String> {
        refs_str
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn serialize_references(refs: &[String]) -> String {
        serde_json::to_string(refs).unwrap_or_else(|_| "[]".to_string())
    }
}

#[async_trait::async_trait(?Send)]
impl Database for SqliteDatabase {
    async fn add_note(&self, note: CreateNote) -> Result<i64, Error> {
        let conn = self.conn.lock().unwrap();

        let refs_json = Self::serialize_references(&note.references);

        // Insert the note
        conn.execute(
            "INSERT INTO notes (title, body, \"references\") VALUES (?1, ?2, ?3)",
            params![note.title, note.body, refs_json],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        let note_id = conn.last_insert_rowid();

        // Insert tags
        for tag in &note.tags {
            conn.execute(
                "INSERT INTO tags (name) VALUES (?1) ON CONFLICT (name) DO NOTHING",
                params![tag],
            )
            .map_err(|e| Error::Database(e.to_string()))?;

            conn.execute(
                "INSERT INTO note_tags (note_id, tag_id) SELECT ?1, id FROM tags WHERE name = ?2",
                params![note_id, tag],
            )
            .map_err(|e| Error::Database(e.to_string()))?;
        }

        Ok(note_id)
    }

    async fn get_note(&self, id: i64) -> Result<Option<Note>, Error> {
        let conn = self.conn.lock().unwrap();

        let note = conn
            .query_row(
                "SELECT n.id, n.title, n.body, n.updated_at, n.\"references\", GROUP_CONCAT(t.name) as tags
                 FROM notes n
                 LEFT JOIN note_tags nt ON n.id = nt.note_id
                 LEFT JOIN tags t ON nt.tag_id = t.id
                 WHERE n.id = ?1
                 GROUP BY n.id",
                params![id],
                |row| {
                    Ok(Note {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        body: row.get(2)?,
                        updated_at: row.get(3)?,
                        references: Self::parse_references(row.get(4)?),
                        tags: Self::parse_tags(row.get(5)?),
                    })
                },
            )
            .optional()
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(note)
    }

    async fn list_notes(&self, query: NoteQuery) -> Result<Vec<Note>, Error> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT n.id, n.title, n.body, n.updated_at, n.\"references\", GROUP_CONCAT(t.name) as tags
             FROM notes n
             LEFT JOIN note_tags nt ON n.id = nt.note_id
             LEFT JOIN tags t ON nt.tag_id = t.id",
        );

        let mut conditions = Vec::new();
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(ref tags) = query.tags {
            if !tags.is_empty() {
                let placeholders: Vec<_> = (0..tags.len()).map(|i| format!("?{}", i + 1)).collect();
                conditions.push(format!(
                    "n.id IN (SELECT note_id FROM note_tags nt2 
                              JOIN tags t2 ON nt2.tag_id = t2.id 
                              WHERE t2.name IN ({}))",
                    placeholders.join(",")
                ));
                params_vec.extend(tags.clone());
            }
        }

        if let Some(ref from) = query.from {
            conditions.push(format!("n.updated_at >= ?{}", params_vec.len() + 1));
            params_vec.push(from.clone());
        }

        if let Some(ref to) = query.to {
            conditions.push(format!("n.updated_at <= ?{}", params_vec.len() + 1));
            params_vec.push(to.clone());
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" GROUP BY n.id ORDER BY n.updated_at DESC, n.id DESC");

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|p| p as &dyn rusqlite::ToSql)
            .collect();

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| Error::Database(e.to_string()))?;

        let notes = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(Note {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    body: row.get(2)?,
                    updated_at: row.get(3)?,
                    references: Self::parse_references(row.get(4)?),
                    tags: Self::parse_tags(row.get(5)?),
                })
            })
            .map_err(|e| Error::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(notes)
    }

    async fn count_notes(&self, query: NoteQuery) -> Result<i64, Error> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from("SELECT COUNT(DISTINCT n.id) FROM notes n");

        let mut conditions = Vec::new();
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(ref tags) = query.tags {
            if !tags.is_empty() {
                let placeholders: Vec<_> = (0..tags.len()).map(|i| format!("?{}", i + 1)).collect();
                conditions.push(format!(
                    "n.id IN (SELECT note_id FROM note_tags nt2 
                              JOIN tags t2 ON nt2.tag_id = t2.id 
                              WHERE t2.name IN ({}))",
                    placeholders.join(",")
                ));
                params_vec.extend(tags.clone());
            }
        }

        if let Some(ref from) = query.from {
            conditions.push(format!("n.updated_at >= ?{}", params_vec.len() + 1));
            params_vec.push(from.clone());
        }

        if let Some(ref to) = query.to {
            conditions.push(format!("n.updated_at <= ?{}", params_vec.len() + 1));
            params_vec.push(to.clone());
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|p| p as &dyn rusqlite::ToSql)
            .collect();

        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(count)
    }

    async fn update_note(&self, id: i64, update: UpdateNote) -> Result<bool, Error> {
        let conn = self.conn.lock().unwrap();

        // Check if note exists
        let exists: bool = conn
            .query_row("SELECT 1 FROM notes WHERE id = ?1", params![id], |_| {
                Ok(true)
            })
            .optional()
            .map_err(|e| Error::Database(e.to_string()))?
            .unwrap_or(false);

        if !exists {
            return Ok(false);
        }

        // Update title if provided
        if let Some(ref title) = update.title {
            conn.execute(
                "UPDATE notes SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![title, id],
            )
            .map_err(|e| Error::Database(e.to_string()))?;
        }

        // Update body if provided
        if let Some(ref body) = update.body {
            conn.execute(
                "UPDATE notes SET body = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![body, id],
            )
            .map_err(|e| Error::Database(e.to_string()))?;
        }

        // Update tags if provided
        if let Some(ref tags) = update.tags {
            // Delete existing tags
            conn.execute("DELETE FROM note_tags WHERE note_id = ?1", params![id])
                .map_err(|e| Error::Database(e.to_string()))?;

            // Insert new tags
            for tag in tags {
                conn.execute(
                    "INSERT INTO tags (name) VALUES (?1) ON CONFLICT (name) DO NOTHING",
                    params![tag],
                )
                .map_err(|e| Error::Database(e.to_string()))?;

                conn.execute(
                    "INSERT INTO note_tags (note_id, tag_id) SELECT ?1, id FROM tags WHERE name = ?2",
                    params![id, tag],
                )
                .map_err(|e| Error::Database(e.to_string()))?;
            }

            // Update timestamp
            conn.execute(
                "UPDATE notes SET updated_at = datetime('now') WHERE id = ?1",
                params![id],
            )
            .map_err(|e| Error::Database(e.to_string()))?;
        }

        // Update references if provided
        if let Some(ref references) = update.references {
            let refs_json = Self::serialize_references(references);
            conn.execute(
                "UPDATE notes SET \"references\" = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![refs_json, id],
            )
            .map_err(|e| Error::Database(e.to_string()))?;
        }

        Ok(true)
    }

    async fn delete_note(&self, id: i64) -> Result<bool, Error> {
        let conn = self.conn.lock().unwrap();

        let rows = conn
            .execute("DELETE FROM notes WHERE id = ?1", params![id])
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(rows > 0)
    }

    async fn list_tags(&self) -> Result<Vec<TagCount>, Error> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT t.name, COUNT(nt.note_id) as count
                 FROM tags t
                 LEFT JOIN note_tags nt ON t.id = nt.tag_id
                 GROUP BY t.id
                 HAVING count > 0
                 ORDER BY count DESC, t.name",
            )
            .map_err(|e| Error::Database(e.to_string()))?;

        let tags = stmt
            .query_map([], |row| {
                Ok(TagCount {
                    name: row.get(0)?,
                    count: row.get(1)?,
                })
            })
            .map_err(|e| Error::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(tags)
    }

    async fn grep(
        &self,
        pattern: &str,
        tags: Option<&[String]>,
        case_sensitive: bool,
    ) -> Result<Vec<Note>, Error> {
        let conn = self.conn.lock().unwrap();

        // Build regex
        let regex = if case_sensitive {
            Regex::new(pattern).map_err(|e| Error::Validation(format!("invalid regex: {}", e)))?
        } else {
            Regex::new(&format!("(?i){}", pattern))
                .map_err(|e| Error::Validation(format!("invalid regex: {}", e)))?
        };

        // Query all notes (with tag filter if provided)
        let mut sql = String::from(
            "SELECT n.id, n.title, n.body, n.updated_at, n.\"references\", GROUP_CONCAT(t.name) as tags
             FROM notes n
             LEFT JOIN note_tags nt ON n.id = nt.note_id
             LEFT JOIN tags t ON nt.tag_id = t.id",
        );

        let mut params_vec: Vec<String> = Vec::new();

        if let Some(tag_list) = tags {
            if !tag_list.is_empty() {
                let placeholders: Vec<_> =
                    (0..tag_list.len()).map(|i| format!("?{}", i + 1)).collect();
                sql.push_str(&format!(
                    " WHERE n.id IN (SELECT note_id FROM note_tags nt2 
                                     JOIN tags t2 ON nt2.tag_id = t2.id 
                                     WHERE t2.name IN ({}))",
                    placeholders.join(",")
                ));
                params_vec.extend(tag_list.iter().cloned());
            }
        }

        sql.push_str(" GROUP BY n.id ORDER BY n.updated_at DESC, n.id DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|p| p as &dyn rusqlite::ToSql)
            .collect();

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| Error::Database(e.to_string()))?;

        let all_notes: Vec<Note> = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(Note {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    body: row.get(2)?,
                    updated_at: row.get(3)?,
                    references: Self::parse_references(row.get(4)?),
                    tags: Self::parse_tags(row.get(5)?),
                })
            })
            .map_err(|e| Error::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::Database(e.to_string()))?;

        // Filter by regex
        let matching: Vec<Note> = all_notes
            .into_iter()
            .filter(|note| regex.is_match(&note.title) || regex.is_match(&note.body))
            .collect();

        Ok(matching)
    }
}
