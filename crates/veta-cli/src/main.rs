//! Veta CLI - memory and knowledge base for agents.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use veta_core::{parse_human_date, NoteQuery, UpdateNote, VetaService};
use veta_sqlite::SqliteDatabase;

const VETA_DIR: &str = ".veta";
const DB_FILE: &str = "db.sqlite";

#[derive(Parser)]
#[command(name = "veta", about = "Memory and knowledge base for agents", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new veta database in the current directory
    Init {
        /// Delete existing database and reinitialize
        #[arg(long)]
        reinitialize: bool,
    },
    /// Add a new note
    Add {
        /// Note title
        #[arg(long)]
        title: String,
        /// Comma-separated tags
        #[arg(long)]
        tags: String,
        /// Note body (reads from stdin if not provided)
        #[arg(long)]
        body: Option<String>,
        /// Comma-separated references (source code paths, URLs, documentation links, etc.)
        #[arg(long)]
        references: Option<String>,
    },
    /// List notes
    Ls {
        /// Filter by comma-separated tags (optional)
        tags: Option<String>,
        /// Filter notes updated after this time (e.g., "2 days ago", "2024-01-01")
        #[arg(long)]
        from: Option<String>,
        /// Filter notes updated before this time
        #[arg(long)]
        to: Option<String>,
        /// Number of notes to show (0 for all)
        #[arg(short = 'n', long, default_value = "100")]
        head: i64,
    },
    /// Show one or more notes
    Show {
        /// Comma-separated note IDs
        ids: String,
        /// Only show the first n lines of each note body
        #[arg(short = 'n', long)]
        head: Option<usize>,
    },
    /// Edit a note
    Edit {
        /// Note ID
        id: i64,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New comma-separated tags
        #[arg(long)]
        tags: Option<String>,
        /// New body (reads from stdin if not provided and stdin is not a tty)
        #[arg(long)]
        body: Option<String>,
        /// New comma-separated references (source code paths, URLs, documentation links, etc.)
        #[arg(long)]
        references: Option<String>,
    },
    /// Delete one or more notes
    Rm {
        /// Comma-separated note IDs
        ids: String,
    },
    /// List all tags
    Tags,
    /// Search notes with regular expressions
    Grep {
        /// Search pattern (regex)
        pattern: String,
        /// Filter by comma-separated tags
        #[arg(long)]
        tags: Option<String>,
        /// Case-sensitive search
        #[arg(short = 'C', long)]
        case_sensitive: bool,
    },
}

/// Find the .veta directory by searching up from current directory
fn find_veta_dir() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        let veta_path = current.join(VETA_DIR);
        if veta_path.is_dir() {
            return Some(veta_path);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Get the database path, or error if not initialized
fn get_db_path() -> Result<PathBuf> {
    match find_veta_dir() {
        Some(veta_dir) => Ok(veta_dir.join(DB_FILE)),
        None => bail!("No .veta directory found. Run 'veta init' to initialize a new database."),
    }
}

/// Check if the database is valid/uncorrupted
fn check_database_integrity(path: &Path) -> Result<bool> {
    use rusqlite::Connection;

    let conn = match Connection::open(path) {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };

    // Run SQLite integrity check
    let result: Result<String, _> = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0));

    match result {
        Ok(status) => Ok(status == "ok"),
        Err(_) => Ok(false),
    }
}

/// Attempt to recover a corrupted database
fn attempt_recovery(path: &Path) -> Result<bool> {
    use rusqlite::Connection;

    // Try to open and run recovery
    let conn = match Connection::open(path) {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };

    // Try to recover using VACUUM (can fix some issues)
    if conn.execute("VACUUM", []).is_ok() {
        // Re-check integrity
        let result: Result<String, _> =
            conn.query_row("PRAGMA integrity_check", [], |row| row.get(0));
        if let Ok(status) = result {
            if status == "ok" {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Open the database, checking for corruption
fn open_database(path: &Path) -> Result<SqliteDatabase> {
    // Check if file exists
    if !path.exists() {
        bail!(
            "Database file not found at {}. Run 'veta init' to create a new database.",
            path.display()
        );
    }

    // Check integrity
    if !check_database_integrity(path)? {
        eprintln!("Warning: Database corruption detected. Attempting recovery...");

        if attempt_recovery(path)? {
            eprintln!("Recovery successful!");
        } else {
            // Backup the corrupted file
            let backup_path = path.with_extension("sqlite.corrupted");
            if std::fs::rename(path, &backup_path).is_ok() {
                bail!(
                    "Database is corrupted and could not be recovered.\n\
                     The corrupted file has been moved to: {}\n\
                     Run 'veta init' to create a new database.",
                    backup_path.display()
                );
            } else {
                bail!(
                    "Database is corrupted and could not be recovered.\n\
                     Please remove {} and run 'veta init' to create a new database.",
                    path.display()
                );
            }
        }
    }

    let db = SqliteDatabase::open(path).context("Failed to open database")?;
    db.run_migrations().context("Failed to run migrations")?;
    Ok(db)
}

fn parse_tags(tags: &str) -> Vec<String> {
    tags.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_ids(ids: &str) -> Result<Vec<i64>> {
    ids.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<i64>().context(format!("Invalid note ID: {}", s)))
        .collect()
}

fn read_stdin() -> Result<String> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .context("Failed to read from stdin")?;
    Ok(buf)
}

fn is_stdin_tty() -> bool {
    atty::is(atty::Stream::Stdin)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Commands::Init { reinitialize } = cli.command {
        let veta_dir = PathBuf::from(VETA_DIR);
        let db_path = veta_dir.join(DB_FILE);

        if veta_dir.exists() {
            if db_path.exists() {
                if reinitialize {
                    std::fs::remove_file(&db_path).context("Failed to remove existing database")?;
                } else {
                    bail!("Veta is already initialized in this directory. Use --reinitialize to delete and recreate.");
                }
            }
        } else {
            std::fs::create_dir_all(&veta_dir).context("Failed to create .veta directory")?;
        }

        let db = SqliteDatabase::open(&db_path).context("Failed to create database")?;
        db.run_migrations()
            .context("Failed to initialize database schema")?;

        if reinitialize {
            println!("Reinitialized veta database in {}", db_path.display());
        } else {
            println!("Initialized veta database in {}", db_path.display());
        }
        return Ok(());
    }

    // All other commands need the database
    let db_path = get_db_path()?;
    let db = open_database(&db_path)?;
    let service = VetaService::new(db);

    match cli.command {
        Commands::Init { .. } => unreachable!(),

        Commands::Add {
            title,
            tags,
            body,
            references,
        } => {
            let body = match body {
                Some(b) => b,
                None => read_stdin()?,
            };
            let tags = parse_tags(&tags);
            let references = references.map(|r| parse_tags(&r)).unwrap_or_default();
            let id = service.add_note(title, body, tags, references).await?;
            println!("Added note {}", id);
        }

        Commands::Ls {
            tags,
            from,
            to,
            head,
        } => {
            let from = from.map(|s| parse_human_date(&s)).transpose()?;
            let to = to.map(|s| parse_human_date(&s)).transpose()?;
            let tags = tags.map(|t| parse_tags(&t));

            let query = NoteQuery {
                tags: tags.clone(),
                from: from.clone(),
                to: to.clone(),
                limit: Some(head),
            };
            let notes = service.list_notes(query).await?;
            let num_notes = notes.len() as i64;

            for note in notes {
                println!(
                    "{}: {} ({}) -- {}",
                    note.id, note.title, note.updated_at, note.body_preview
                );
            }

            // Show truncation message if there are more notes
            if head > 0 && num_notes >= head {
                let count_query = NoteQuery {
                    tags,
                    from,
                    to,
                    limit: None,
                };
                let total = service.count_notes(count_query).await?;
                if total > head {
                    println!("[Showing the latest {}/{} notes]", head, total);
                }
            }
        }

        Commands::Show { ids, head } => {
            let ids = parse_ids(&ids)?;
            if ids.is_empty() {
                eprintln!("No note IDs provided");
                std::process::exit(1);
            }

            let mut not_found = Vec::new();
            let mut first = true;

            for id in &ids {
                match service.get_note(*id).await? {
                    Some(note) => {
                        if !first {
                            println!("\n{}\n", "=".repeat(40));
                        }
                        first = false;

                        println!("# {}\n", note.title);

                        // Apply --head if specified
                        if let Some(n) = head {
                            let lines: Vec<&str> = note.body.lines().take(n).collect();
                            println!("{}", lines.join("\n"));
                            if note.body.lines().count() > n {
                                println!("...");
                            }
                        } else {
                            println!("{}", note.body);
                        }

                        println!("\n---\n");
                        println!("Last modified: {}", note.updated_at);
                        println!("Tags: {}", note.tags.join(","));
                        if !note.references.is_empty() {
                            println!("References:");
                            for reference in &note.references {
                                println!("  - {}", reference);
                            }
                        }
                    }
                    None => {
                        not_found.push(*id);
                    }
                }
            }

            if !not_found.is_empty() {
                if !first {
                    eprintln!(); // Add spacing after last note
                }
                for id in &not_found {
                    eprintln!("Note {} not found", id);
                }
                std::process::exit(1);
            }
        }

        Commands::Tags => {
            let tags = service.list_tags().await?;
            for tag in tags {
                let noun = if tag.count == 1 { "note" } else { "notes" };
                println!("{} ({} {})", tag.name, tag.count, noun);
            }
        }

        Commands::Grep {
            pattern,
            tags,
            case_sensitive,
        } => {
            let tags = tags.map(|t| parse_tags(&t));
            let notes = service.grep(&pattern, tags, case_sensitive).await?;
            for note in notes {
                println!(
                    "{}: {} ({}) -- {}",
                    note.id, note.title, note.updated_at, note.body_preview
                );
            }
        }

        Commands::Edit {
            id,
            title,
            tags,
            body,
            references,
        } => {
            let body = if body.is_none() && !is_stdin_tty() {
                Some(read_stdin()?)
            } else {
                body
            };

            let update = UpdateNote {
                title,
                body,
                tags: tags.map(|t| parse_tags(&t)),
                references: references.map(|r| parse_tags(&r)),
            };

            let mut updated_fields = Vec::new();
            if update.title.is_some() {
                updated_fields.push("title");
            }
            if update.body.is_some() {
                updated_fields.push("body");
            }
            if update.tags.is_some() {
                updated_fields.push("tags");
            }
            if update.references.is_some() {
                updated_fields.push("references");
            }

            if updated_fields.is_empty() {
                eprintln!("Nothing to update");
                std::process::exit(1);
            }

            if service.update_note(id, update).await? {
                println!("Edited note {}: Updated {}", id, updated_fields.join(", "));
            } else {
                eprintln!("Note {} not found", id);
                std::process::exit(1);
            }
        }

        Commands::Rm { ids } => {
            let ids = parse_ids(&ids)?;
            if ids.is_empty() {
                eprintln!("No note IDs provided");
                std::process::exit(1);
            }

            let mut deleted = Vec::new();
            let mut not_found = Vec::new();

            for id in &ids {
                if service.delete_note(*id).await? {
                    deleted.push(*id);
                } else {
                    not_found.push(*id);
                }
            }

            for id in &deleted {
                println!("Deleted note {}", id);
            }

            if !not_found.is_empty() {
                for id in &not_found {
                    eprintln!("Note {} not found", id);
                }
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
