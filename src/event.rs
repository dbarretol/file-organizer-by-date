use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

/// What `execute` and `undo` report while they work. Values are *owned* so
/// they can be sent through a channel from a worker thread (e.g. to a GUI).
#[derive(Debug)]
pub enum Event {
    FolderCreated {
        folder_name: String,
    },
    /// Undo only: a folder created by the run was left empty and was
    /// removed.
    FolderRemoved {
        folder_name: String,
    },
    /// The name was already taken at the destination and another one will
    /// be used. `late` means the conflict appeared after the initial check
    /// (another process created the file: TOCTOU) and the move is retried.
    Conflict {
        original: OsString,
        renamed_to: OsString,
        late: bool,
    },
    Moved {
        from: PathBuf,
        to: PathBuf,
    },
    /// It disappeared between the scan and the move: not our failure, so it
    /// counts as skipped.
    SourceMissing {
        path: PathBuf,
    },
    Failed {
        source: PathBuf,
        failure: Failure,
    },
}

/// Why a file couldn't be moved. Each one counts as an error.
#[derive(Debug)]
pub enum Failure {
    /// The destination path exists but is not a folder (e.g. a file named
    /// "Y2026-M07" with no extension).
    DestNotADirectory {
        dest_dir: PathBuf,
    },
    CreateDir {
        dest_dir: PathBuf,
        error: io::Error,
    },
    NoFileName,
    ResolveConflict(io::Error),
    /// The destination was still taken after several retries.
    DestinationBusy {
        attempts: u32,
    },
    /// The copy to the destination finished, but the original couldn't be
    /// deleted: the file is now duplicated (source and destination).
    CopiedButOriginalRemains {
        dest: PathBuf,
        error: io::Error,
    },
    /// Both rename and copy failed.
    Move {
        rename: io::Error,
        copy: io::Error,
    },
}
