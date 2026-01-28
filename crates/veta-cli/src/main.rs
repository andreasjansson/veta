//! Veta CLI - memory and knowledge base for agents.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, Read};
use veta_core::{NoteQuery, UpdateNote, VetaService};
use veta_sqlite::SqliteDatabase;

#[derive(Parser)]
#[command(name = "veta", about = "Memory and knowledge base for agents")]
struct Cli {
    /// Database path (default: ~/.veta/notes.db)
    #[arg(long, env = "VETA_DB_PATH")]
    db: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
    },
    /// List notes
    Ls {
        /// Filter by comma-separated tags
        #[arg(long)]
        tags: Option<String>,
        /// Filter notes updated after this time
        #[arg(long)]
        from: Option<String>,
        /// Filter notes updated before this time
        #[arg(long)]
        to: Option<String>,
        /// Number of notes to show (0 for all)
        #[arg(short = 'n', long, default_value = "20")]
        head: i64,
    },
    /// Show a note
    Show {
        /// Note ID
        id: i64,
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
    },
    /// Delete a note
    Delete {
        /// Note ID
        id: i64,
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

fn get_default_db_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    format!("{}/.veta/notes.db", home)
}

fn parse_tags(tags: &str) -> Vec<String> {
    tags.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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

    let db_path = cli.db.unwrap_or_else(get_default_db_path);

    // Ensure directory exists
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent).context("Failed to create database directory")?;
    }

    let db = SqliteDatabase::open(&db_path).context("Failed to open database")?;
    db.run_migrations().context("Failed to run migrations")?;
    let service = VetaService::new(db);

    match cli.command {
        Commands::Add { title, tags, body } => {
            let body = match body {
                Some(b) => b,
                None => read_stdin()?,
            };
            let tags = parse_tags(&tags);
            let id = service.add_note(title, body, tags).await?;
            println!("Added note {}", id);
        }

        Commands::Ls {
            tags,
            from,
            to,
            head,
        } => {
            let query = NoteQuery {
                tags: tags.map(|t| parse_tags(&t)),
                from,
                to,
                limit: Some(head),
            };
            let notes = service.list_notes(query).await?;
            for note in notes {
                println!(
                    "{}: {} ({}) -- {}",
                    note.id, note.title, note.updated_at, note.body_preview
                );
            }
        }

        Commands::Show { id } => match service.get_note(id).await? {
            Some(note) => {
                println!("# {}\n", note.title);
                println!("{}", note.body);
                println!("\n---\n");
                println!("Last modified: {}", note.updated_at);
                println!("Tags: {}", note.tags.join(","));
            }
            None => {
                eprintln!("Note {} not found", id);
                std::process::exit(1);
            }
        },

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

        Commands::Delete { id } => {
            if service.delete_note(id).await? {
                println!("Deleted note {}", id);
            } else {
                eprintln!("Note {} not found", id);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
