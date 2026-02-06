//! File-based storage implementation for Veta.
//!
//! Stores notes as JSON files with symlinks for tag organization:
//!
//! ```text
//! .veta/
//!   .lock                    # Lock file for atomic operations
//!   notes/
//!     1.json
//!     2.json
//!   tags/
//!     architecture/
//!       1.json → ../notes/1.json
//!     testing/
//!       2.json → ../notes/2.json
//! ```

use chrono::Utc;
use fs2::FileExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use veta_core::{CreateNote, Database, Error, Note, NoteQuery, TagCount, UpdateNote};

/// A note as stored on disk (without ID or tags - tags come from symlinks).
#[derive(Debug, Serialize, Deserialize)]
struct NoteFile {
    title: String,
    body: String,
    #[serde(default)]
    references: Vec<String>,
    modified: String,
}

/// File-based database implementation.
pub struct FilesDatabase {
    root: PathBuf,
}

impl FilesDatabase {
    /// Open a file-based database at the given .veta directory.
    pub fn open<P: AsRef<Path>>(root: P) -> Result<Self, Error> {
        let root = root.as_ref().to_path_buf();

        // Create directories if they don't exist
        let notes_dir = root.join("notes");
        let tags_dir = root.join("tags");

        fs::create_dir_all(&notes_dir)
            .map_err(|e| Error::Database(format!("Failed to create notes dir: {}", e)))?;
        fs::create_dir_all(&tags_dir)
            .map_err(|e| Error::Database(format!("Failed to create tags dir: {}", e)))?;

        Ok(Self { root })
    }

    /// Acquire an exclusive lock on the database.
    fn lock(&self) -> Result<FileLock, Error> {
        let lock_path = self.root.join(".lock");
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&lock_path)
            .map_err(|e| Error::Database(format!("Failed to open lock file: {}", e)))?;

        file.lock_exclusive()
            .map_err(|e| Error::Database(format!("Failed to acquire lock: {}", e)))?;

        Ok(FileLock { file })
    }

    /// Get the path to a note file.
    fn note_path(&self, id: i64) -> PathBuf {
        self.root.join("notes").join(format!("{}.json", id))
    }

    /// Get the next available note ID.
    /// Uses a counter file to ensure IDs always increase, even after deletions.
    fn next_id(&self) -> Result<i64, Error> {
        let counter_path = self.root.join("counter");

        // Read existing counter, or scan notes dir if counter doesn't exist
        let current_max = if counter_path.exists() {
            let contents = fs::read_to_string(&counter_path)
                .map_err(|e| Error::Database(format!("Failed to read counter: {}", e)))?;
            contents.trim().parse::<i64>().unwrap_or(0)
        } else {
            // Initial setup: scan notes directory for max ID
            let notes_dir = self.root.join("notes");
            let mut max_id: i64 = 0;

            if let Ok(entries) = fs::read_dir(&notes_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(stem) = path.file_stem() {
                        if let Some(stem_str) = stem.to_str() {
                            if let Ok(id) = stem_str.parse::<i64>() {
                                max_id = max_id.max(id);
                            }
                        }
                    }
                }
            }
            max_id
        };

        let next_id = current_max + 1;

        // Write the new counter value
        fs::write(&counter_path, next_id.to_string())
            .map_err(|e| Error::Database(format!("Failed to write counter: {}", e)))?;

        Ok(next_id)
    }

    /// Read a note file from disk.
    fn read_note_file(&self, id: i64) -> Result<Option<NoteFile>, Error> {
        let path = self.note_path(id);

        if !path.exists() {
            return Ok(None);
        }

        let mut file = File::open(&path)
            .map_err(|e| Error::Database(format!("Failed to open note: {}", e)))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| Error::Database(format!("Failed to read note: {}", e)))?;

        let note: NoteFile = serde_json::from_str(&contents)
            .map_err(|e| Error::Database(format!("Failed to parse note: {}", e)))?;

        Ok(Some(note))
    }

    /// Write a note file to disk atomically.
    fn write_note_file(&self, id: i64, note: &NoteFile) -> Result<(), Error> {
        let path = self.note_path(id);
        let temp_path = self.root.join("notes").join(format!("{}.json.tmp", id));

        let contents = serde_json::to_string_pretty(note)
            .map_err(|e| Error::Database(format!("Failed to serialize note: {}", e)))?;

        // Write to temp file
        let mut file = File::create(&temp_path)
            .map_err(|e| Error::Database(format!("Failed to create temp file: {}", e)))?;

        file.write_all(contents.as_bytes())
            .map_err(|e| Error::Database(format!("Failed to write temp file: {}", e)))?;

        file.sync_all()
            .map_err(|e| Error::Database(format!("Failed to sync temp file: {}", e)))?;

        // Atomic rename
        fs::rename(&temp_path, &path)
            .map_err(|e| Error::Database(format!("Failed to rename temp file: {}", e)))?;

        Ok(())
    }

    /// Get tags for a note by scanning tag directories.
    fn get_note_tags(&self, id: i64) -> Result<Vec<String>, Error> {
        let tags_dir = self.root.join("tags");
        let mut tags = Vec::new();

        let entries = match fs::read_dir(&tags_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(tags),
        };

        for entry in entries {
            let entry =
                entry.map_err(|e| Error::Database(format!("Failed to read dir entry: {}", e)))?;
            let path = entry.path();

            if path.is_dir() {
                let symlink_path = path.join(format!("{}.json", id));
                if self.symlink_exists(&symlink_path) {
                    if let Some(tag_name) = path.file_name().and_then(|n| n.to_str()) {
                        tags.push(tag_name.to_string());
                    }
                }
            }
        }

        tags.sort();
        Ok(tags)
    }

    /// Check if a symlink exists (works on both Unix and Windows).
    fn symlink_exists(&self, path: &Path) -> bool {
        // On Unix, this checks for symlinks
        // On Windows, this checks for either real symlinks or our text-file-based symlinks
        if path.exists() {
            return true;
        }

        // Check if it's a symlink that points to a non-existent target
        path.symlink_metadata().is_ok()
    }

    /// Create a symlink (or text file on Windows if symlinks fail).
    fn create_symlink(&self, target: &Path, link: &Path) -> Result<(), Error> {
        // Remove existing if any
        let _ = fs::remove_file(link);

        // Calculate relative path from link to target
        let link_parent = link
            .parent()
            .ok_or_else(|| Error::Database("Symlink path has no parent".to_string()))?;

        // We want a relative path like "../notes/1.json"
        let relative_target =
            pathdiff::diff_paths(target, link_parent).unwrap_or_else(|| target.to_path_buf());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&relative_target, link)
                .map_err(|e| Error::Database(format!("Failed to create symlink: {}", e)))?;
        }

        #[cfg(windows)]
        {
            // Try to create a real symlink first
            match std::os::windows::fs::symlink_file(&relative_target, link) {
                Ok(()) => {}
                Err(_) => {
                    // Fall back to text file containing the path
                    let mut file = File::create(link).map_err(|e| {
                        Error::Database(format!("Failed to create link file: {}", e))
                    })?;
                    // Use forward slashes for consistency
                    let path_str = relative_target.to_string_lossy().replace('\\', "/");
                    file.write_all(path_str.as_bytes()).map_err(|e| {
                        Error::Database(format!("Failed to write link file: {}", e))
                    })?;
                }
            }
        }

        Ok(())
    }

    /// Resolve a symlink to get the actual file path (handles Windows text-file symlinks).
    #[allow(dead_code)]
    fn resolve_symlink(&self, link: &Path) -> Result<PathBuf, Error> {
        // Try to read as a real symlink first
        if let Ok(target) = fs::read_link(link) {
            // Resolve relative to the link's parent directory
            let link_parent = link
                .parent()
                .ok_or_else(|| Error::Database("Symlink path has no parent".to_string()))?;
            return Ok(link_parent.join(target));
        }

        // Fall back to reading as a text file (Windows without symlink support)
        let mut file = File::open(link)
            .map_err(|e| Error::Database(format!("Failed to open link file: {}", e)))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| Error::Database(format!("Failed to read link file: {}", e)))?;

        let link_parent = link
            .parent()
            .ok_or_else(|| Error::Database("Symlink path has no parent".to_string()))?;

        Ok(link_parent.join(contents.trim()))
    }

    /// Update symlinks for a note's tags.
    fn update_tags(&self, id: i64, tags: &[String]) -> Result<(), Error> {
        let tags_dir = self.root.join("tags");
        let note_path = self.note_path(id);

        // Remove all existing tag symlinks for this note
        if let Ok(entries) = fs::read_dir(&tags_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let symlink_path = path.join(format!("{}.json", id));
                    let _ = fs::remove_file(&symlink_path);
                }
            }
        }

        // Create new tag symlinks
        for tag in tags {
            let tag_dir = tags_dir.join(tag);
            fs::create_dir_all(&tag_dir)
                .map_err(|e| Error::Database(format!("Failed to create tag dir: {}", e)))?;

            let symlink_path = tag_dir.join(format!("{}.json", id));
            self.create_symlink(&note_path, &symlink_path)?;
        }

        // Clean up empty tag directories
        self.cleanup_empty_tag_dirs()?;

        Ok(())
    }

    /// Remove empty tag directories.
    fn cleanup_empty_tag_dirs(&self) -> Result<(), Error> {
        let tags_dir = self.root.join("tags");

        if let Ok(entries) = fs::read_dir(&tags_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Check if directory is empty
                    if let Ok(mut dir_entries) = fs::read_dir(&path) {
                        if dir_entries.next().is_none() {
                            let _ = fs::remove_dir(&path);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// List all note IDs in the notes directory.
    fn list_note_ids(&self) -> Result<Vec<i64>, Error> {
        let notes_dir = self.root.join("notes");
        let mut ids = Vec::new();

        let entries = fs::read_dir(&notes_dir)
            .map_err(|e| Error::Database(format!("Failed to read notes dir: {}", e)))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| Error::Database(format!("Failed to read dir entry: {}", e)))?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    if let Some(stem_str) = stem.to_str() {
                        if let Ok(id) = stem_str.parse::<i64>() {
                            ids.push(id);
                        }
                    }
                }
            }
        }

        Ok(ids)
    }

    /// List note IDs that have a specific tag.
    fn list_note_ids_with_tag(&self, tag: &str) -> Result<Vec<i64>, Error> {
        let tag_dir = self.root.join("tags").join(tag);
        let mut ids = Vec::new();

        let entries = match fs::read_dir(&tag_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(ids),
        };

        for entry in entries {
            let entry =
                entry.map_err(|e| Error::Database(format!("Failed to read dir entry: {}", e)))?;
            let path = entry.path();

            if let Some(stem) = path.file_stem() {
                if let Some(stem_str) = stem.to_str() {
                    if let Ok(id) = stem_str.parse::<i64>() {
                        ids.push(id);
                    }
                }
            }
        }

        Ok(ids)
    }

    /// Load a full Note from disk (note file + tags from symlinks).
    fn load_note(&self, id: i64) -> Result<Option<Note>, Error> {
        let note_file = match self.read_note_file(id)? {
            Some(nf) => nf,
            None => return Ok(None),
        };

        let tags = self.get_note_tags(id)?;

        Ok(Some(Note {
            id,
            title: note_file.title,
            body: note_file.body,
            references: note_file.references,
            tags,
            updated_at: note_file.modified,
        }))
    }

    /// Get current timestamp in ISO 8601 format.
    fn now() -> String {
        Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

/// RAII guard for file locking.
struct FileLock {
    file: File,
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[async_trait::async_trait(?Send)]
impl Database for FilesDatabase {
    async fn add_note(&self, note: CreateNote) -> Result<i64, Error> {
        let _lock = self.lock()?;

        let id = self.next_id()?;
        let note_file = NoteFile {
            title: note.title,
            body: note.body,
            references: note.references,
            modified: Self::now(),
        };

        self.write_note_file(id, &note_file)?;
        self.update_tags(id, &note.tags)?;

        Ok(id)
    }

    async fn get_note(&self, id: i64) -> Result<Option<Note>, Error> {
        self.load_note(id)
    }

    async fn list_notes(&self, query: NoteQuery) -> Result<Vec<Note>, Error> {
        // Get candidate note IDs based on tag filter
        let ids = if let Some(ref tags) = query.tags {
            if tags.is_empty() {
                self.list_note_ids()?
            } else {
                // Get IDs that have ANY of the specified tags
                let mut all_ids = std::collections::HashSet::new();
                for tag in tags {
                    for id in self.list_note_ids_with_tag(tag)? {
                        all_ids.insert(id);
                    }
                }
                all_ids.into_iter().collect()
            }
        } else {
            self.list_note_ids()?
        };

        // Load all notes
        let mut notes = Vec::new();
        for id in ids {
            if let Some(note) = self.load_note(id)? {
                // Apply date filters
                if let Some(ref from) = query.from {
                    if note.updated_at < *from {
                        continue;
                    }
                }
                if let Some(ref to) = query.to {
                    if note.updated_at > *to {
                        continue;
                    }
                }
                notes.push(note);
            }
        }

        // Sort by updated_at DESC, then by id DESC
        notes.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.id.cmp(&a.id))
        });

        // Apply limit
        if let Some(limit) = query.limit {
            if limit > 0 {
                notes.truncate(limit as usize);
            }
        }

        Ok(notes)
    }

    async fn count_notes(&self, query: NoteQuery) -> Result<i64, Error> {
        // Reuse list_notes logic but just count (could be optimized)
        let notes = self
            .list_notes(NoteQuery {
                limit: None,
                ..query
            })
            .await?;
        Ok(notes.len() as i64)
    }

    async fn update_note(&self, id: i64, update: UpdateNote) -> Result<bool, Error> {
        let _lock = self.lock()?;

        // Check if note exists
        let mut note_file = match self.read_note_file(id)? {
            Some(nf) => nf,
            None => return Ok(false),
        };

        // Apply updates
        if let Some(title) = update.title {
            note_file.title = title;
        }
        if let Some(body) = update.body {
            note_file.body = body;
        }
        if let Some(references) = update.references {
            note_file.references = references;
        }

        // Update modified timestamp
        note_file.modified = Self::now();

        // Write back
        self.write_note_file(id, &note_file)?;

        // Update tags if provided
        if let Some(tags) = update.tags {
            self.update_tags(id, &tags)?;
        }

        Ok(true)
    }

    async fn delete_note(&self, id: i64) -> Result<bool, Error> {
        let _lock = self.lock()?;

        let path = self.note_path(id);
        if !path.exists() {
            return Ok(false);
        }

        // Remove the note file
        fs::remove_file(&path)
            .map_err(|e| Error::Database(format!("Failed to delete note: {}", e)))?;

        // Remove all tag symlinks for this note
        let tags_dir = self.root.join("tags");
        if let Ok(entries) = fs::read_dir(&tags_dir) {
            for entry in entries.flatten() {
                let tag_path = entry.path();
                if tag_path.is_dir() {
                    let symlink_path = tag_path.join(format!("{}.json", id));
                    let _ = fs::remove_file(&symlink_path);
                }
            }
        }

        // Clean up empty tag directories
        self.cleanup_empty_tag_dirs()?;

        Ok(true)
    }

    async fn list_tags(&self) -> Result<Vec<TagCount>, Error> {
        let tags_dir = self.root.join("tags");
        let mut tag_counts = Vec::new();

        let entries = match fs::read_dir(&tags_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(tag_counts),
        };

        for entry in entries {
            let entry =
                entry.map_err(|e| Error::Database(format!("Failed to read dir entry: {}", e)))?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(tag_name) = path.file_name().and_then(|n| n.to_str()) {
                    // Count symlinks in the tag directory
                    let count = fs::read_dir(&path)
                        .map(|entries| entries.count())
                        .unwrap_or(0);

                    if count > 0 {
                        tag_counts.push(TagCount {
                            name: tag_name.to_string(),
                            count: count as i64,
                        });
                    }
                }
            }
        }

        // Sort by count DESC, then by name ASC
        tag_counts.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));

        Ok(tag_counts)
    }

    async fn grep(
        &self,
        pattern: &str,
        tags: Option<&[String]>,
        case_sensitive: bool,
    ) -> Result<Vec<Note>, Error> {
        // Build regex
        let regex = if case_sensitive {
            Regex::new(pattern).map_err(|e| Error::Validation(format!("invalid regex: {}", e)))?
        } else {
            Regex::new(&format!("(?i){}", pattern))
                .map_err(|e| Error::Validation(format!("invalid regex: {}", e)))?
        };

        // Get candidate note IDs based on tag filter
        let ids = if let Some(tag_list) = tags {
            if tag_list.is_empty() {
                self.list_note_ids()?
            } else {
                // Get IDs that have ANY of the specified tags
                let mut all_ids = std::collections::HashSet::new();
                for tag in tag_list {
                    for id in self.list_note_ids_with_tag(tag)? {
                        all_ids.insert(id);
                    }
                }
                all_ids.into_iter().collect()
            }
        } else {
            self.list_note_ids()?
        };

        // Load and filter notes
        let mut notes = Vec::new();
        for id in ids {
            if let Some(note) = self.load_note(id)? {
                if regex.is_match(&note.title) || regex.is_match(&note.body) {
                    notes.push(note);
                }
            }
        }

        // Sort by updated_at DESC, then by id DESC
        notes.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.id.cmp(&a.id))
        });

        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FilesDatabase) {
        let temp_dir = TempDir::new().unwrap();
        let db = FilesDatabase::open(temp_dir.path()).unwrap();
        (temp_dir, db)
    }

    #[tokio::test]
    async fn test_add_and_get_note() {
        let (_temp, db) = setup();

        let id = db
            .add_note(CreateNote {
                title: "Test note".to_string(),
                body: "Test body".to_string(),
                tags: vec!["tag1".to_string(), "tag2".to_string()],
                references: vec!["ref1".to_string()],
            })
            .await
            .unwrap();

        assert_eq!(id, 1);

        let note = db.get_note(id).await.unwrap().unwrap();
        assert_eq!(note.title, "Test note");
        assert_eq!(note.body, "Test body");
        assert_eq!(note.tags, vec!["tag1", "tag2"]);
        assert_eq!(note.references, vec!["ref1"]);
    }

    #[tokio::test]
    async fn test_list_notes_by_tag() {
        let (_temp, db) = setup();

        db.add_note(CreateNote {
            title: "Note 1".to_string(),
            body: "Body 1".to_string(),
            tags: vec!["alpha".to_string()],
            references: vec![],
        })
        .await
        .unwrap();

        db.add_note(CreateNote {
            title: "Note 2".to_string(),
            body: "Body 2".to_string(),
            tags: vec!["beta".to_string()],
            references: vec![],
        })
        .await
        .unwrap();

        db.add_note(CreateNote {
            title: "Note 3".to_string(),
            body: "Body 3".to_string(),
            tags: vec!["alpha".to_string(), "beta".to_string()],
            references: vec![],
        })
        .await
        .unwrap();

        let alpha_notes = db
            .list_notes(NoteQuery {
                tags: Some(vec!["alpha".to_string()]),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(alpha_notes.len(), 2);
        assert!(alpha_notes.iter().any(|n| n.title == "Note 1"));
        assert!(alpha_notes.iter().any(|n| n.title == "Note 3"));
    }

    #[tokio::test]
    async fn test_update_note() {
        let (_temp, db) = setup();

        let id = db
            .add_note(CreateNote {
                title: "Original".to_string(),
                body: "Original body".to_string(),
                tags: vec!["old".to_string()],
                references: vec![],
            })
            .await
            .unwrap();

        db.update_note(
            id,
            UpdateNote {
                title: Some("Updated".to_string()),
                body: Some("Updated body".to_string()),
                tags: Some(vec!["new".to_string()]),
                references: None,
            },
        )
        .await
        .unwrap();

        let note = db.get_note(id).await.unwrap().unwrap();
        assert_eq!(note.title, "Updated");
        assert_eq!(note.body, "Updated body");
        assert_eq!(note.tags, vec!["new"]);
    }

    #[tokio::test]
    async fn test_delete_note() {
        let (_temp, db) = setup();

        let id = db
            .add_note(CreateNote {
                title: "To delete".to_string(),
                body: "Body".to_string(),
                tags: vec!["temp".to_string()],
                references: vec![],
            })
            .await
            .unwrap();

        assert!(db.delete_note(id).await.unwrap());
        assert!(db.get_note(id).await.unwrap().is_none());
        assert!(!db.delete_note(id).await.unwrap());
    }

    #[tokio::test]
    async fn test_list_tags() {
        let (_temp, db) = setup();

        db.add_note(CreateNote {
            title: "Note 1".to_string(),
            body: "Body".to_string(),
            tags: vec!["alpha".to_string()],
            references: vec![],
        })
        .await
        .unwrap();

        db.add_note(CreateNote {
            title: "Note 2".to_string(),
            body: "Body".to_string(),
            tags: vec!["alpha".to_string(), "beta".to_string()],
            references: vec![],
        })
        .await
        .unwrap();

        let tags = db.list_tags().await.unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "alpha");
        assert_eq!(tags[0].count, 2);
        assert_eq!(tags[1].name, "beta");
        assert_eq!(tags[1].count, 1);
    }

    #[tokio::test]
    async fn test_grep() {
        let (_temp, db) = setup();

        db.add_note(CreateNote {
            title: "Hello world".to_string(),
            body: "This is a test".to_string(),
            tags: vec!["greeting".to_string()],
            references: vec![],
        })
        .await
        .unwrap();

        db.add_note(CreateNote {
            title: "Goodbye".to_string(),
            body: "Farewell".to_string(),
            tags: vec!["farewell".to_string()],
            references: vec![],
        })
        .await
        .unwrap();

        let matches = db.grep("hello", None, false).await.unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].title, "Hello world");

        let matches = db.grep("HELLO", None, true).await.unwrap();
        assert_eq!(matches.len(), 0);
    }

    #[tokio::test]
    async fn test_concurrent_adds() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Create the database and its directories
        FilesDatabase::open(&root).unwrap();

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let root = root.clone();
                thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        let db = FilesDatabase::open(&root).unwrap();
                        db.add_note(CreateNote {
                            title: format!("Note {}", i),
                            body: format!("Body {}", i),
                            tags: vec!["concurrent".to_string()],
                            references: vec![],
                        })
                        .await
                        .unwrap()
                    })
                })
            })
            .collect();

        let ids: Vec<i64> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All IDs should be unique
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique_ids.len(), 10);

        // All notes should exist
        let db = FilesDatabase::open(&root).unwrap();
        let notes = db.list_notes(NoteQuery::default()).await.unwrap();
        assert_eq!(notes.len(), 10);
    }

    #[tokio::test]
    async fn test_concurrent_add_and_delete() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Create the database
        let db = FilesDatabase::open(&root).unwrap();

        // Add some initial notes
        for i in 0..5 {
            db.add_note(CreateNote {
                title: format!("Initial {}", i),
                body: "Body".to_string(),
                tags: vec!["initial".to_string()],
                references: vec![],
            })
            .await
            .unwrap();
        }

        // Spawn threads that add and delete concurrently
        let root_add = root.clone();
        let root_delete = root.clone();

        let add_handle = thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = FilesDatabase::open(&root_add).unwrap();
                let mut ids = Vec::new();
                for i in 0..5 {
                    let id = db
                        .add_note(CreateNote {
                            title: format!("Added {}", i),
                            body: "Body".to_string(),
                            tags: vec!["added".to_string()],
                            references: vec![],
                        })
                        .await
                        .unwrap();
                    ids.push(id);
                }
                ids
            })
        });

        let delete_handle = thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = FilesDatabase::open(&root_delete).unwrap();
                for i in 1..=5 {
                    let _ = db.delete_note(i).await;
                }
            })
        });

        let added_ids = add_handle.join().unwrap();
        delete_handle.join().unwrap();

        // Check that added notes exist (they shouldn't have been deleted as they have higher IDs)
        let db = FilesDatabase::open(&root).unwrap();
        for id in added_ids {
            assert!(
                db.get_note(id).await.unwrap().is_some(),
                "Note {} should exist",
                id
            );
        }
    }
}
