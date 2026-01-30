//! Embedded database migrations for Veta.
//!
//! Migrations are versioned and run automatically on first database access.
//! The schema version is tracked in the `_veta_meta` table.

/// Current schema version. Increment when adding new migrations.
pub const SCHEMA_VERSION: i64 = 2;

/// A database migration with version number and SQL statements.
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub statements: &'static [&'static str],
}

/// All migrations in order. Each migration should be idempotent where possible.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        statements: &[
            "CREATE TABLE IF NOT EXISTS _veta_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
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
        ],
    },
    Migration {
        version: 2,
        name: "add_references",
        statements: &[
            // ALTER TABLE doesn't support IF NOT EXISTS, so we check in code
            "ALTER TABLE notes ADD COLUMN \"references\" TEXT NOT NULL DEFAULT '[]'",
        ],
    },
];

/// Get migrations that need to be applied given the current version.
pub fn get_pending_migrations(current_version: i64) -> Vec<&'static Migration> {
    MIGRATIONS
        .iter()
        .filter(|m| m.version > current_version)
        .collect()
}
