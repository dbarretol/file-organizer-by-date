//! Core of File Organizer by Date: plan and carry out moving the loose files
//! of a folder into subfolders by date, and undo it.
//!
//! It never prints or reads stdin: it returns data ([`Plan`], [`Summary`])
//! and emits [`Event`]s, so any frontend (CLI, GUI) decides how to show them.

mod event;
mod executor;
mod filter;
mod fs_ops;
mod grouping;
mod journal;
mod options;
mod plan;

pub use event::{Event, Failure};
pub use executor::{execute, Summary};
pub use filter::ExtensionFilter;
pub use grouping::{folder_for, Granularity};
pub use journal::{undo, Journal, JournalMove, JournalRecorder, UndoResult, JOURNAL_FILE_NAME};
pub use options::{DateSource, Options};
pub use plan::{scan, Plan, PlannedMove, ScanWarning};
