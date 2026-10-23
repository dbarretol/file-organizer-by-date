use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::event::{Event, Failure};
use crate::fs_ops::{move_file, resolve_conflict, MoveError};
use crate::plan::{Plan, PlannedMove};

/// Retries when another process takes the destination after the name was resolved.
const MAX_MOVE_ATTEMPTS: u32 = 5;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub moved: u32,
    pub skipped: u32,
    pub errors: u32,
}

impl Summary {
    /// No errors. Skipped files (ones that disappeared) don't count as
    /// failures.
    pub fn is_success(&self) -> bool {
        self.errors == 0
    }
}

/// Carries out the plan, emitting an `Event` for every relevant step.
/// Never overwrites an existing file.
pub fn execute(plan: &Plan, on_event: &mut dyn FnMut(Event)) -> Summary {
    let mut summary = Summary::default();

    for planned in &plan.moves {
        match move_one(&plan.base, planned, on_event) {
            Ok(Some(dest)) => {
                summary.moved += 1;
                on_event(Event::Moved {
                    from: planned.source.clone(),
                    to: dest,
                });
            }
            Ok(None) => {
                summary.skipped += 1;
                on_event(Event::SourceMissing {
                    path: planned.source.clone(),
                });
            }
            Err(failure) => {
                summary.errors += 1;
                on_event(Event::Failed {
                    source: planned.source.clone(),
                    failure,
                });
            }
        }
    }

    summary
}

/// Moves one file into its destination folder. `Ok(None)` if the source is gone.
fn move_one(
    base: &Path,
    planned: &PlannedMove,
    on_event: &mut dyn FnMut(Event),
) -> Result<Option<PathBuf>, Failure> {
    let dest_dir = ensure_dest_dir(base, &planned.folder_name, on_event)?;

    if !planned.source.exists() {
        return Ok(None);
    }
    let file_name = planned.source.file_name().ok_or(Failure::NoFileName)?;

    move_with_retries(&planned.source, &dest_dir, file_name, on_event).map(Some)
}

fn ensure_dest_dir(
    base: &Path,
    folder_name: &str,
    on_event: &mut dyn FnMut(Event),
) -> Result<PathBuf, Failure> {
    let dest_dir = base.join(folder_name);
    if dest_dir.is_dir() {
        return Ok(dest_dir);
    }
    // It may exist as a file: report it instead of trying to move into
    // something that isn't a folder.
    if dest_dir.exists() {
        return Err(Failure::DestNotADirectory { dest_dir });
    }
    if let Err(error) = fs::create_dir_all(&dest_dir) {
        return Err(Failure::CreateDir { dest_dir, error });
    }
    on_event(Event::FolderCreated {
        folder_name: folder_name.to_owned(),
    });
    Ok(dest_dir)
}

/// The exists() check is only a hint: move_file never overwrites, so if
/// another process creates the destination right after the check
/// (TOCTOU), another name is resolved and the move is retried.
pub(crate) fn move_with_retries(
    source: &Path,
    dest_dir: &Path,
    file_name: &OsStr,
    on_event: &mut dyn FnMut(Event),
) -> Result<PathBuf, Failure> {
    let mut dest = dest_dir.join(file_name);
    if dest.exists() {
        dest = next_free_name(dest_dir, file_name, &dest, false, on_event)?;
    }

    let mut attempts = 0;
    loop {
        match move_file(source, &dest) {
            Ok(()) => return Ok(dest),
            Err(MoveError::DestinationExists) => {
                attempts += 1;
                if attempts > MAX_MOVE_ATTEMPTS {
                    return Err(Failure::DestinationBusy { attempts });
                }
                dest = next_free_name(dest_dir, file_name, &dest, true, on_event)?;
            }
            Err(MoveError::CopiedButOriginalRemains(error)) => {
                return Err(Failure::CopiedButOriginalRemains { dest, error })
            }
            Err(MoveError::Failed { rename, copy }) => return Err(Failure::Move { rename, copy }),
        }
    }
}

fn next_free_name(
    dest_dir: &Path,
    file_name: &OsStr,
    taken: &Path,
    late: bool,
    on_event: &mut dyn FnMut(Event),
) -> Result<PathBuf, Failure> {
    let resolved = resolve_conflict(dest_dir, file_name).map_err(Failure::ResolveConflict)?;
    on_event(Event::Conflict {
        original: taken.file_name().unwrap_or_default().to_owned(),
        renamed_to: resolved.file_name().unwrap_or_default().to_owned(),
        late,
    });
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::Options;
    use crate::plan::scan;

    #[test]
    fn execute_moves_files_and_renames_on_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        fs::write(base.join("a.txt"), "new").unwrap();
        fs::write(base.join("b.txt"), "").unwrap();

        let plan = scan(base, &Options::default()).unwrap();
        let folder = plan.moves[0].folder_name.clone();
        // "a.txt" already exists at the destination: it must be renamed, not overwritten
        fs::create_dir(base.join(&folder)).unwrap();
        fs::write(base.join(&folder).join("a.txt"), "previous").unwrap();

        let mut events = Vec::new();
        let summary = execute(&plan, &mut |e| events.push(e));

        assert_eq!(
            summary,
            Summary {
                moved: 2,
                skipped: 0,
                errors: 0
            }
        );
        let dest = base.join(&folder);
        assert_eq!(fs::read_to_string(dest.join("a.txt")).unwrap(), "previous");
        assert_eq!(fs::read_to_string(dest.join("a_1.txt")).unwrap(), "new");
        assert!(dest.join("b.txt").exists());
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Conflict { late: false, .. })));
        // The folder already existed: it isn't reported as created
        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::FolderCreated { .. })));
    }

    #[test]
    fn execute_counts_vanished_source_as_skipped() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "").unwrap();
        let plan = scan(dir.path(), &Options::default()).unwrap();
        fs::remove_file(dir.path().join("a.txt")).unwrap();

        let summary = execute(&plan, &mut |_| {});

        assert_eq!(summary.skipped, 1);
        assert!(summary.is_success());
    }

    #[test]
    fn execute_fails_when_dest_folder_is_a_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "").unwrap();
        let plan = scan(dir.path(), &Options::default()).unwrap();
        fs::write(dir.path().join(&plan.moves[0].folder_name), "").unwrap();

        let mut events = Vec::new();
        let summary = execute(&plan, &mut |e| events.push(e));

        assert_eq!(summary.errors, 1);
        assert!(matches!(
            events[..],
            [Event::Failed {
                failure: Failure::DestNotADirectory { .. },
                ..
            }]
        ));
    }
}
