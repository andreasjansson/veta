//! Veta CLI - memory and knowledge base for agents.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, Read};
use std::path::PathBuf;
use veta_core::{parse_human_date, Database, NoteQuery, UpdateNote, VetaService};
use veta_files::FilesDatabase;

const VETA_DIR: &str = ".veta";
const LEGACY_DB_FILE: &str = "db.sqlite";

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

/// Get the veta directory path, or error if not initialized
fn get_veta_dir() -> Result<PathBuf> {
    match find_veta_dir() {
        Some(veta_dir) => Ok(veta_dir),
        None => bail!("No .veta directory found. Run 'veta init' to initialize a new database."),
    }
}

/// Check if there's a legacy SQLite database that needs migration
fn has_legacy_sqlite(veta_dir: &PathBuf) -> bool {
    veta_dir.join(LEGACY_DB_FILE).exists()
}

/// Migrate from SQLite to file-based storage
async fn migrate_from_sqlite(veta_dir: &PathBuf) -> Result<()> {
    use veta_sqlite::SqliteDatabase;

    let sqlite_path = veta_dir.join(LEGACY_DB_FILE);
    eprintln!(
        "Migrating from SQLite database to file-based storage..."
    );

    // Open the SQLite database
    let sqlite_db = SqliteDatabase::open(&sqlite_path)
        .context("Failed to open legacy SQLite database")?;

    // Create the new file-based database
    let _files_db = FilesDatabase::open(veta_dir)
        .context("Failed to create file-based database")?;

    // Get all notes from SQLite
    let notes = sqlite_db
        .list_notes(NoteQuery {
            limit: None,
            ..Default::default()
        })
        .await
        .context("Failed to list notes from SQLite")?;

    eprintln!("Migrating {} notes...", notes.len());

    // We need to preserve the original IDs, but FilesDatabase.add_note generates new IDs.
    // So we'll directly write the note files with the correct IDs.
    // This is a bit of a hack, but it's only for migration.
    
    for note in notes {
        // Create the note file directly to preserve the ID
        let note_path = veta_dir.join("notes").join(format!("{}.json", note.id));
        
        let note_file = serde_json::json!({
            "title": note.title,
            "body": note.body,
            "references": note.references,
            "modified": note.updated_at,
        });
        
        let contents = serde_json::to_string_pretty(&note_file)
            .context("Failed to serialize note")?;
        
        std::fs::write(&note_path, contents)
            .context(format!("Failed to write note {}", note.id))?;
        
        // Create tag symlinks
        for tag in &note.tags {
            let tag_dir = veta_dir.join("tags").join(tag);
            std::fs::create_dir_all(&tag_dir)
                .context(format!("Failed to create tag directory: {}", tag))?;
            
            let symlink_path = tag_dir.join(format!("{}.json", note.id));
            let relative_target = format!("../../notes/{}.json", note.id);
            
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&relative_target, &symlink_path)
                    .context(format!("Failed to create symlink for note {} tag {}", note.id, tag))?;
            }
            
            #[cfg(windows)]
            {
                // Try real symlink first, fall back to text file
                if std::os::windows::fs::symlink_file(&relative_target, &symlink_path).is_err() {
                    std::fs::write(&symlink_path, &relative_target)
                        .context(format!("Failed to create link file for note {} tag {}", note.id, tag))?;
                }
            }
        }
    }

    // Remove the old SQLite database
    std::fs::remove_file(&sqlite_path)
        .context("Failed to remove old SQLite database")?;

    eprintln!("Migration complete! SQLite database has been removed.");

    Ok(())
}

/// Open the file-based database, migrating from SQLite if needed
async fn open_database(veta_dir: &PathBuf) -> Result<FilesDatabase> {
    // Check for legacy SQLite and migrate if needed
    if has_legacy_sqlite(veta_dir) {
        migrate_from_sqlite(veta_dir).await?;
    }

    let db = FilesDatabase::open(veta_dir).context("Failed to open database")?;
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

        if veta_dir.exists() {
            if reinitialize {
                // Remove everything in .veta directory
                std::fs::remove_dir_all(&veta_dir)
                    .context("Failed to remove existing .veta directory")?;
            } else {
                // Check if already initialized (has notes dir or sqlite db)
                let has_notes = veta_dir.join("notes").exists();
                let has_sqlite = veta_dir.join(LEGACY_DB_FILE).exists();
                if has_notes || has_sqlite {
                    bail!("Veta is already initialized in this directory. Use --reinitialize to delete and recreate.");
                }
            }
        }

        // Create the file-based database structure
        let _db = FilesDatabase::open(&veta_dir).context("Failed to create database")?;

        if reinitialize {
            println!("Reinitialized veta database in {}", veta_dir.display());
        } else {
            println!("Initialized veta database in {}", veta_dir.display());
        }
        return Ok(());
    }

    // All other commands need the database
    let veta_dir = get_veta_dir()?;
    let db = open_database(&veta_dir).await?;
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
