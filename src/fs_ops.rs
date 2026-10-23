//! Delicate disk operations: move without ever overwriting, withstand
//! race conditions (TOCTOU) and keep the mtime in the copy fallback.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Highest numeric suffix (`_1`, `_2`…) tried when resolving a conflict.
const MAX_CONFLICT_SUFFIX: u32 = 10_000;

#[derive(Debug)]
pub(crate) enum MoveError {
    /// The destination already exists: move_file never overwrites. The
    /// caller must resolve another name and retry.
    DestinationExists,
    /// The copy to the destination finished, but the original couldn't be
    /// deleted: the file is now duplicated (source and destination).
    CopiedButOriginalRemains(io::Error),
    /// Couldn't move: both rename and copy failed.
    Failed { rename: io::Error, copy: io::Error },
}

pub(crate) fn move_file(source: &Path, dest: &Path) -> Result<(), MoveError> {
    // fs::rename silently overwrites the destination on ALL platforms (on
    // Windows it uses MoveFileExW with MOVEFILE_REPLACE_EXISTING), so the
    // name is first claimed atomically with create_new; rename then only
    // replaces that placeholder, which is already ours. If the placeholder
    // fails for another reason (e.g. permissions), rename/copy will fail
    // too and report the real error.
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
    {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(MoveError::DestinationExists)
        }
        Err(_) => {}
    }

    let rename_err = match fs::rename(source, dest) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    // The placeholder is still at dest (rename didn't happen) and it's ours:
    // copy_then_remove fills it in, or removes it if the copy fails.
    // If this remove fails (e.g. an antivirus holding the handle),
    // copy_then_remove will see AlreadyExists and the retry will use another
    // name, leaving this empty 0-byte placeholder behind. Accepted risk: it's
    // harmless, and the alternative (aborting the move) would be worse.
    let _ = fs::remove_file(dest);
    copy_then_remove(source, dest, rename_err)
}

/// Fallback when rename isn't possible (e.g. destination on another volume).
/// Opens the destination with create_new so it never overwrites a file
/// created by another process between the conflict check and this point.
fn copy_then_remove(source: &Path, dest: &Path, rename_err: io::Error) -> Result<(), MoveError> {
    let mut dest_file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
    {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(MoveError::DestinationExists)
        }
        Err(copy_err) => {
            return Err(MoveError::Failed {
                rename: rename_err,
                copy: copy_err,
            })
        }
    };

    let copy_result = (|| {
        let mut src = fs::File::open(source)?;
        let metadata = src.metadata()?;
        io::copy(&mut src, &mut dest_file)?;
        // Preserve what fs::rename would have kept: without this the copied
        // file would get mtime = now, corrupting the very data this tool
        // organizes by.
        dest_file.set_modified(metadata.modified()?)?;
        dest_file.set_permissions(metadata.permissions())?;
        Ok::<(), io::Error>(())
    })();
    drop(dest_file);

    match copy_result {
        Ok(_) => fs::remove_file(source).map_err(MoveError::CopiedButOriginalRemains),
        Err(copy_err) => {
            // Don't leave a half-copied destination behind
            let _ = fs::remove_file(dest);
            Err(MoveError::Failed {
                rename: rename_err,
                copy: copy_err,
            })
        }
    }
}

/// First free name `<stem>_<n><.ext>` inside `dest_dir`.
/// Works with `OsStr` end to end: a name that isn't valid UTF-8 keeps its
/// original bytes instead of turning into `�`.
pub(crate) fn resolve_conflict(dest_dir: &Path, file_name: &OsStr) -> io::Result<PathBuf> {
    let path = Path::new(file_name);
    let stem = path.file_stem().unwrap_or_default();
    let extension = path.extension();

    for i in 1..=MAX_CONFLICT_SUFFIX {
        let mut new_name = stem.to_os_string();
        new_name.push(format!("_{i}"));
        if let Some(ext) = extension {
            new_name.push(".");
            new_name.push(ext);
        }
        let candidate = dest_dir.join(new_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("No se pudo resolver el conflicto de nombre tras {MAX_CONFLICT_SUFFIX} intentos"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_conflict_picks_first_free_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(dir.join("a.txt"), "").unwrap();
        fs::write(dir.join("a_1.txt"), "").unwrap();

        let resolved = resolve_conflict(dir, OsStr::new("a.txt")).unwrap();
        assert_eq!(resolved.file_name().unwrap(), "a_2.txt");

        // File without an extension
        fs::write(dir.join("README"), "").unwrap();
        let resolved = resolve_conflict(dir, OsStr::new("README")).unwrap();
        assert_eq!(resolved.file_name().unwrap(), "README_1");
    }

    #[cfg(unix)]
    #[test]
    fn resolve_conflict_preserves_non_utf8_names() {
        use std::ffi::OsString;
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let dir = tempfile::tempdir().unwrap();
        let name = OsStr::from_bytes(b"\xffdata.txt");

        let resolved = resolve_conflict(dir.path(), name).unwrap();
        let expected = OsString::from_vec(b"\xffdata_1.txt".to_vec());
        assert_eq!(resolved.file_name().unwrap(), expected);
    }

    #[cfg(windows)]
    #[test]
    fn resolve_conflict_preserves_non_utf16_names() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let dir = tempfile::tempdir().unwrap();
        // Lone surrogate: valid on NTFS, not representable in UTF-8
        let wide = |s: &str| {
            let mut v = vec![0xD800u16];
            v.extend(s.encode_utf16());
            OsString::from_wide(&v)
        };

        let resolved = resolve_conflict(dir.path(), &wide("data.txt")).unwrap();
        assert_eq!(resolved.file_name().unwrap(), wide("data_1.txt"));
    }

    #[test]
    fn move_file_moves_when_dest_is_free() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let dest = dir.path().join("b.txt");
        fs::write(&source, "content").unwrap();

        assert!(move_file(&source, &dest).is_ok());
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "content");
    }

    #[test]
    fn move_file_never_overwrites_existing_dest() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let dest = dir.path().join("b.txt");
        fs::write(&source, "new").unwrap();
        fs::write(&dest, "original untouched").unwrap();

        // Simulates the TOCTOU: the destination appeared after the conflict
        // check. move_file must refuse to overwrite.
        assert!(matches!(
            move_file(&source, &dest),
            Err(MoveError::DestinationExists)
        ));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "original untouched");
        assert_eq!(fs::read_to_string(&source).unwrap(), "new");
    }

    #[test]
    fn copy_fallback_preserves_source_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let dest = dir.path().join("b.txt");
        fs::write(&source, "content").unwrap();

        // Old mtime, clearly different from "now" (2001-09-09)
        let old_mtime =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_modified(old_mtime)
            .unwrap();

        // On Windows this works because an explicit set_modified suppresses the
        // implicit last-write update when the handle is closed; if
        // copy_then_remove ever set the mtime by path after closing, this test
        // would catch the regression.
        let rename_err = io::Error::other("simulated");
        assert!(copy_then_remove(&source, &dest, rename_err).is_ok());
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "content");
        assert_eq!(fs::metadata(&dest).unwrap().modified().unwrap(), old_mtime);
    }

    #[test]
    fn copy_fallback_never_overwrites_existing_dest() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let dest = dir.path().join("b.txt");
        fs::write(&source, "new").unwrap();
        fs::write(&dest, "original untouched").unwrap();

        let rename_err = io::Error::other("simulated");
        assert!(matches!(
            copy_then_remove(&source, &dest, rename_err),
            Err(MoveError::DestinationExists)
        ));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "original untouched");
    }
}
