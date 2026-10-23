//! All the text the CLI shows, built from the core's data.

use std::io;
use std::path::Path;

use file_organizer_by_date::{
    DateSource, Event, Failure, Journal, Plan, ScanWarning, Summary, JOURNAL_FILE_NAME,
};

/// Maximum files listed per folder in the preview.
const MAX_PREVIEW_PER_GROUP: usize = 10;

/// Maximum moves listed in the --undo preview.
const MAX_UNDO_PREVIEW: usize = 20;

const SEPARATOR: &str = "─────────────────────────────────────────────";

pub fn banner() {
    // The version comes from Cargo.toml so it never drifts, and the frame is
    // sized to the text so any length (0.4.0, 0.10.0…) stays centered.
    let title = format!("📂 File Organizer by Date v{}", env!("CARGO_PKG_VERSION"));
    let subtitle = "Organize files into folders by date";

    // Terminal columns: the emoji counts as 1 char but takes 2 columns
    let title_cols = title.chars().count() + 1;
    let subtitle_cols = subtitle.chars().count();
    let inner = title_cols.max(subtitle_cols) + 4;

    let centered = |text: &str, cols: usize| {
        let left = (inner - cols) / 2;
        format!(
            "{}{}{}",
            " ".repeat(left),
            text,
            " ".repeat(inner - cols - left)
        )
    };

    println!("╔{}╗", "═".repeat(inner));
    println!("║{}║", centered(&title, title_cols));
    println!("║{}║", centered(subtitle, subtitle_cols));
    println!("╚{}╝", "═".repeat(inner));
    println!();
}

pub fn quote_hint() {
    eprintln!("If the path contains spaces, wrap it in quotes:");
    eprintln!("     file-organizer-by-date \"C:\\My Folder\"");
}

pub fn invalid_dir(path: &Path) {
    eprintln!("✘ '{}' doesn't exist or is not a folder.", path.display());
}

pub fn scan_error(error: &io::Error) {
    eprintln!("✘ Error while scanning: {error}");
}

/// One line of output and the stream it goes to. Keeping the text separate
/// from printing lets the log (`--log`) reuse it.
pub struct Line {
    pub text: String,
    pub to_stderr: bool,
}

impl Line {
    fn out(text: String) -> Self {
        Line {
            text,
            to_stderr: false,
        }
    }

    fn err(text: String) -> Self {
        Line {
            text,
            to_stderr: true,
        }
    }

    pub fn print(&self) {
        if self.to_stderr {
            eprintln!("{}", self.text);
        } else {
            println!("{}", self.text);
        }
    }
}

pub fn warning_line(warning: &ScanWarning) -> Line {
    match warning {
        ScanWarning::UnreadableEntry(e) => {
            Line::err(format!("⏭️  Skipping an unreadable entry: {e}"))
        }
        ScanWarning::Symlink(path) => {
            Line::out(format!("⏭️  Skipping symbolic link: {}", path.display()))
        }
        ScanWarning::UnreadableDir { path, error } => Line::err(format!(
            "⏭️  Skipping folder '{}' (couldn't read it): {}",
            path.display(),
            error
        )),
        ScanWarning::NoDate {
            path,
            source,
            error,
        } => Line::err(format!(
            "⏭️  Skipping '{}' (couldn't read its {} date): {}",
            path.display(),
            match source {
                DateSource::Modified => "modification",
                DateSource::Created => "creation",
            },
            error
        )),
    }
}

pub fn nothing_to_do() {
    println!("\n⚠️  No files to organize were found in that folder.");
}

/// Shows the moves grouped by destination folder, with a count per group
/// and at most `MAX_PREVIEW_PER_GROUP` files listed for each. Relies on
/// `Plan::moves` being sorted by `folder_name`.
pub fn preview(plan: &Plan) {
    let moves = &plan.moves;

    println!("\n🔍 Preview:");
    println!("{SEPARATOR}");

    let mut start = 0;
    while start < moves.len() {
        let folder = &moves[start].folder_name;
        let count = moves[start..]
            .iter()
            .take_while(|m| &m.folder_name == folder)
            .count();

        println!("   📁 {}/  ({} file(s))", folder, count);
        for planned in &moves[start..start + count.min(MAX_PREVIEW_PER_GROUP)] {
            // Relative to the base: in recursive mode this tells apart
            // same-named files from different subfolders; otherwise it's
            // just the file name.
            let shown = planned
                .source
                .strip_prefix(&plan.base)
                .unwrap_or(&planned.source);
            println!("      {}", shown.display());
        }
        if count > MAX_PREVIEW_PER_GROUP {
            println!("      ... and {} more", count - MAX_PREVIEW_PER_GROUP);
        }

        start += count;
    }

    println!("{SEPARATOR}");
    println!("📊 {} file(s) will be moved in total.\n", moves.len());
}

pub fn cancelled() {
    println!("❌ Operation cancelled. No files were moved.");
}

pub fn event_line(event: &Event) -> Line {
    match event {
        Event::FolderCreated { folder_name } => {
            Line::out(format!("📁 Folder created: {folder_name}"))
        }
        Event::FolderRemoved { folder_name } => {
            Line::out(format!("🗑️  Empty folder removed: {folder_name}"))
        }
        Event::Conflict {
            original,
            renamed_to,
            late: false,
        } => Line::out(format!(
            "⚠️  Conflict: '{}' already exists → renaming to '{}'",
            original.to_string_lossy(),
            renamed_to.to_string_lossy()
        )),
        Event::Conflict {
            original,
            renamed_to,
            late: true,
        } => Line::out(format!(
            "⚠️  Conflict: '{}' appeared at the destination → retrying as '{}'",
            original.to_string_lossy(),
            renamed_to.to_string_lossy()
        )),
        Event::Moved { from, to } => Line::out(format!("✔ {} → {}", from.display(), to.display())),
        Event::SourceMissing { path } => {
            Line::out(format!("⏭️  File no longer exists: {}", path.display()))
        }
        Event::Failed { source, failure } => Line::err(failure_text(source, failure)),
    }
}

fn failure_text(source: &Path, failure: &Failure) -> String {
    let source = source.display();
    match failure {
        Failure::DestNotADirectory { dest_dir } => format!(
            "✘ '{}' already exists but is not a folder; skipping '{}'",
            dest_dir.display(),
            source
        ),
        Failure::CreateDir { dest_dir, error } => format!(
            "✘ Error creating folder '{}': {}",
            dest_dir.display(),
            error
        ),
        Failure::NoFileName => {
            format!("✘ Skipping '{source}': the path has no file name")
        }
        Failure::ResolveConflict(error) => format!("✘ Error resolving name conflict: {error}"),
        Failure::DestinationBusy { attempts } => format!(
            "✘ Couldn't move '{source}': the destination was still taken after {attempts} attempts"
        ),
        Failure::CopiedButOriginalRemains { dest, error } => format!(
            "⚠️  '{}' was copied to '{}' but the original couldn't be deleted (now duplicated): {}",
            source,
            dest.display(),
            error
        ),
        Failure::Move { rename, copy } => {
            format!("✘ Error moving '{source}': {copy} (rename failed with: {rename})")
        }
    }
}

pub fn summary(summary: &Summary) {
    println!("\n✅ Result:");
    println!("   Files moved:  {}", summary.moved);
    println!("   Skipped:      {}", summary.skipped);
    println!("   Errors:       {}", summary.errors);
}

pub fn separator() {
    println!("\n{SEPARATOR}");
}

pub fn goodbye() {
    println!("👋 Goodbye!");
}

pub fn undo_hint(base: &Path) {
    println!(
        "↩️  To undo this run: file-organizer-by-date \"{}\" --undo",
        base.display()
    );
}

pub fn unrecorded(count: usize) {
    eprintln!("⚠️  {count} move(s) with non-UTF-8 names couldn't be recorded and can't be undone.");
}

pub fn log_error(error: &io::Error) {
    eprintln!("✘ Couldn't open the log file: {error}");
}

pub fn journal_error(error: &io::Error) {
    eprintln!("✘ Error with the undo record ({JOURNAL_FILE_NAME}): {error}");
}

pub fn nothing_to_undo() {
    println!("\n⚠️  There is nothing to undo in that folder.");
}

/// Lists what will be restored: `current location → original location`.
pub fn undo_preview(journal: &Journal) {
    // If someone edited the date by hand, show it as is
    let when = chrono::DateTime::parse_from_rfc3339(&journal.created_at)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|_| journal.created_at.clone());
    println!("\n↩️  Undo the run from {when}:");
    println!("{SEPARATOR}");
    for entry in journal.moves.iter().take(MAX_UNDO_PREVIEW) {
        println!("   {} → {}", entry.to.display(), entry.from.display());
    }
    if journal.moves.len() > MAX_UNDO_PREVIEW {
        println!("   ... and {} more", journal.moves.len() - MAX_UNDO_PREVIEW);
    }
    println!("{SEPARATOR}");
    println!(
        "📊 {} file(s) will be restored to their original location.\n",
        journal.moves.len()
    );
}
