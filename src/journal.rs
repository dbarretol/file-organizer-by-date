//! Registro de la última ejecución en la carpeta base, para poder deshacerla.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use chrono::{Local, SecondsFormat};
use serde::{Deserialize, Serialize};

use crate::event::{Event, Failure};
use crate::executor::{move_with_retries, Summary};

/// Nombre del journal dentro de la carpeta base. El escaneo lo ignora.
pub const JOURNAL_FILE_NAME: &str = ".file-organizer-journal.json";

const JOURNAL_VERSION: u32 = 1;

/// Movimientos de la última ejecución. Las rutas son relativas a la carpeta
/// base: el journal sigue siendo válido si se mueve o renombra la carpeta.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Journal {
    version: u32,
    /// RFC 3339, hora local.
    pub created_at: String,
    /// En el orden en que se hicieron.
    pub moves: Vec<JournalMove>,
    /// Carpetas creadas por la ejecución, en orden de creación.
    pub created_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

impl Journal {
    fn new() -> Self {
        Journal {
            version: JOURNAL_VERSION,
            created_at: Local::now().to_rfc3339_opts(SecondsFormat::Secs, false),
            ..Journal::default()
        }
    }

    pub fn path(base: &Path) -> PathBuf {
        base.join(JOURNAL_FILE_NAME)
    }

    /// `Ok(None)` si la carpeta no tiene journal.
    pub fn load(base: &Path) -> io::Result<Option<Journal>> {
        let text = match fs::read_to_string(Self::path(base)) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let journal: Journal = serde_json::from_str(&text).map_err(invalid_data)?;
        journal.validate()?;
        Ok(Some(journal))
    }

    /// Escribe a un temporal y lo renombra: un corte a mitad de escritura no
    /// deja un journal corrupto.
    fn save(&self, base: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(invalid_data)?;
        let tmp = base.join(format!("{JOURNAL_FILE_NAME}.tmp"));
        fs::write(&tmp, json)?;
        fs::rename(&tmp, Self::path(base))
    }

    /// Tras un undo: guarda lo que quedó pendiente, o borra el journal si ya
    /// no queda nada por deshacer.
    pub fn persist(&self, base: &Path) -> io::Result<()> {
        if !self.moves.is_empty() {
            return self.save(base);
        }
        match fs::remove_file(Self::path(base)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// El journal es un archivo editable dentro de la carpeta: solo se
    /// aceptan rutas relativas que no salgan de ella (sin `..` ni raíz).
    fn validate(&self) -> io::Result<()> {
        if self.version != JOURNAL_VERSION {
            return Err(invalid_data(format!(
                "versión de journal no soportada: {}",
                self.version
            )));
        }
        let paths = self
            .moves
            .iter()
            .flat_map(|m| [&m.from, &m.to])
            .chain(&self.created_dirs);
        for path in paths {
            let inside = path.components().count() > 0
                && path.components().all(|c| matches!(c, Component::Normal(_)));
            if !inside {
                return Err(invalid_data(format!(
                    "ruta fuera de la carpeta base en el journal: '{}'",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}

fn invalid_data(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

/// Construye el journal a partir de los eventos de `execute`.
pub struct JournalRecorder {
    base: PathBuf,
    journal: Journal,
    unrecorded: usize,
}

impl JournalRecorder {
    pub fn new(base: &Path) -> Self {
        JournalRecorder {
            base: base.to_path_buf(),
            journal: Journal::new(),
            unrecorded: 0,
        }
    }

    pub fn record(&mut self, event: &Event) {
        match event {
            Event::Moved { from, to } => match (self.relative(from), self.relative(to)) {
                (Some(from), Some(to)) => self.journal.moves.push(JournalMove { from, to }),
                _ => self.unrecorded += 1,
            },
            Event::FolderCreated { folder_name } => {
                self.journal.created_dirs.push(PathBuf::from(folder_name))
            }
            _ => {}
        }
    }

    /// JSON solo admite rutas UTF-8: un movimiento con otra ruta no se puede
    /// registrar (ni, por tanto, deshacer).
    fn relative(&self, path: &Path) -> Option<PathBuf> {
        let relative = path.strip_prefix(&self.base).ok()?;
        relative.to_str().map(PathBuf::from)
    }

    /// Movimientos que no quedaron registrados.
    pub fn unrecorded(&self) -> usize {
        self.unrecorded
    }

    /// Guarda el journal, reemplazando el anterior, solo si se movió algo:
    /// una ejecución sin cambios no debe borrar lo que se puede deshacer.
    /// Devuelve si se guardó.
    pub fn save(&self) -> io::Result<bool> {
        if self.journal.moves.is_empty() {
            return Ok(false);
        }
        self.journal.save(&self.base).map(|()| true)
    }
}

pub struct UndoResult {
    pub summary: Summary,
    /// Lo que no se pudo deshacer; `Journal::persist` lo guarda para
    /// reintentarlo o borra el journal si está vacío.
    pub pending: Journal,
}

/// Devuelve cada archivo a su ubicación original, en orden inverso, sin
/// sobrescribir nunca (un nombre ocupado se resuelve con sufijo, como al
/// organizar). Después elimina las carpetas creadas que hayan quedado vacías.
pub fn undo(base: &Path, journal: &Journal, on_event: &mut dyn FnMut(Event)) -> UndoResult {
    let mut summary = Summary::default();
    let mut pending = Journal {
        moves: Vec::new(),
        created_dirs: Vec::new(),
        ..journal.clone()
    };

    for entry in journal.moves.iter().rev() {
        let current = base.join(&entry.to);
        if !current.exists() {
            // Ya no está donde lo dejamos: no hay nada que devolver
            summary.skipped += 1;
            on_event(Event::SourceMissing { path: current });
            continue;
        }
        match restore(base, &current, &entry.from, on_event) {
            Ok(restored) => {
                summary.moved += 1;
                on_event(Event::Moved {
                    from: current,
                    to: restored,
                });
            }
            Err(failure) => {
                summary.errors += 1;
                pending.moves.push(entry.clone());
                on_event(Event::Failed {
                    source: current,
                    failure,
                });
            }
        }
    }
    pending.moves.reverse();

    for dir in journal.created_dirs.iter().rev() {
        // remove_dir solo borra carpetas vacías: lo que el usuario haya
        // añadido después nunca se toca.
        if fs::remove_dir(base.join(dir)).is_ok() {
            on_event(Event::FolderRemoved {
                folder_name: dir.display().to_string(),
            });
        } else if base.join(dir).is_dir() {
            pending.created_dirs.insert(0, dir.clone());
        }
    }

    UndoResult { summary, pending }
}

fn restore(
    base: &Path,
    current: &Path,
    original: &Path,
    on_event: &mut dyn FnMut(Event),
) -> Result<PathBuf, Failure> {
    let file_name = original.file_name().ok_or(Failure::NoFileName)?;
    let dest_dir = base.join(original.parent().unwrap_or(Path::new("")));

    // En modo recursivo la subcarpeta de origen puede haberse borrado
    if !dest_dir.is_dir() {
        if let Err(error) = fs::create_dir_all(&dest_dir) {
            return Err(Failure::CreateDir { dest_dir, error });
        }
        on_event(Event::FolderCreated {
            folder_name: original
                .parent()
                .unwrap_or(Path::new(""))
                .display()
                .to_string(),
        });
    }

    move_with_retries(current, &dest_dir, file_name, on_event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::execute;
    use crate::options::Options;
    use crate::plan::scan;

    fn organize(base: &Path, options: &Options) -> Summary {
        let plan = scan(base, options).unwrap();
        let mut recorder = JournalRecorder::new(base);
        let summary = execute(&plan, &mut |e| recorder.record(&e));
        recorder.save().unwrap();
        summary
    }

    #[test]
    fn undo_restores_files_and_removes_created_folders() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        fs::create_dir(base.join("sub")).unwrap();
        fs::write(base.join("a.txt"), "a").unwrap();
        fs::write(base.join("sub").join("b.txt"), "b").unwrap();
        let options = Options {
            recursive: true,
            ..Options::default()
        };
        assert_eq!(organize(base, &options).moved, 2);
        // Recursivo: la subcarpeta vaciada se borra para probar que se recrea
        fs::remove_dir(base.join("sub")).unwrap();

        let journal = Journal::load(base).unwrap().unwrap();
        assert_eq!(journal.moves.len(), 2);
        let result = undo(base, &journal, &mut |_| {});
        result.pending.persist(base).unwrap();

        assert_eq!(result.summary.moved, 2);
        assert!(result.summary.is_success());
        assert_eq!(fs::read_to_string(base.join("a.txt")).unwrap(), "a");
        assert_eq!(
            fs::read_to_string(base.join("sub").join("b.txt")).unwrap(),
            "b"
        );
        // Solo quedan los archivos restaurados: ni carpetas Y…, ni journal
        let mut left: Vec<_> = fs::read_dir(base)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        left.sort();
        assert_eq!(left, ["a.txt", "sub"]);
    }

    #[test]
    fn undo_keeps_non_empty_created_folders() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        fs::write(base.join("a.txt"), "a").unwrap();
        organize(base, &Options::default());
        let journal = Journal::load(base).unwrap().unwrap();
        let folder = base.join(&journal.created_dirs[0]);
        fs::write(folder.join("del_usuario.txt"), "").unwrap();

        let result = undo(base, &journal, &mut |_| {});

        assert!(result.summary.is_success());
        assert!(folder.join("del_usuario.txt").exists());
    }

    #[test]
    fn run_without_moves_keeps_previous_journal() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        fs::write(base.join("a.txt"), "").unwrap();
        organize(base, &Options::default());

        // Segunda pasada: nada que mover (y el journal no se escanea)
        let second = organize(base, &Options::default());

        assert_eq!(second.moved, 0);
        assert_eq!(Journal::load(base).unwrap().unwrap().moves.len(), 1);
    }

    #[test]
    fn load_rejects_paths_escaping_the_base() {
        let dir = tempfile::tempdir().unwrap();
        let evil = r#"{"version":1,"created_at":"","moves":[{"from":"../fuera.txt","to":"a.txt"}],"created_dirs":[]}"#;
        fs::write(Journal::path(dir.path()), evil).unwrap();

        let err = Journal::load(dir.path()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn load_without_journal_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Journal::load(dir.path()).unwrap().is_none());
    }
}
