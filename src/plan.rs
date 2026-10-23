use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::grouping::folder_for;

/// Resultado de escanear una carpeta: qué se movería y qué se omitió.
/// No toca el disco más allá de leer; ejecutarlo es tarea de `execute`.
#[derive(Debug)]
pub struct Plan {
    pub base: PathBuf,
    /// Ordenado por carpeta destino y luego por ruta origen: preview
    /// agrupado y movimientos en orden predecible.
    pub moves: Vec<PlannedMove>,
    /// En el orden en que se encontraron durante el escaneo.
    pub warnings: Vec<ScanWarning>,
}

#[derive(Debug)]
pub struct PlannedMove {
    pub source: PathBuf,
    pub folder_name: String,
}

/// Entradas que el escaneo omite por un motivo que merece avisarse.
/// (Las carpetas y el propio ejecutable se omiten en silencio.)
#[derive(Debug)]
pub enum ScanWarning {
    UnreadableEntry(io::Error),
    /// Decisión deliberada: los enlaces simbólicos se omiten. is_file() y
    /// metadata() siguen el enlace, así que se clasificaría por la fecha
    /// del destino y moverlo podría romper enlaces relativos.
    Symlink(PathBuf),
    NoModifiedDate {
        path: PathBuf,
        error: io::Error,
    },
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }
}

/// Lee SOLO los archivos sueltos del directorio indicado (sin recursión).
/// Los errores por archivo se registran como avisos en lugar de abortar.
pub fn scan(dir: &Path) -> io::Result<Plan> {
    let self_exe = SelfExe::locate();
    let mut moves = Vec::new();
    let mut warnings = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                warnings.push(ScanWarning::UnreadableEntry(e));
                continue;
            }
        };
        let path = entry.path();

        if path.is_symlink() {
            warnings.push(ScanWarning::Symlink(path));
            continue;
        }

        // Solo archivos, ignorar carpetas por completo
        if !path.is_file() || self_exe.is(&path) {
            continue;
        }

        match fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(modified) => moves.push(PlannedMove {
                folder_name: folder_for(modified.into()),
                source: path,
            }),
            Err(error) => warnings.push(ScanWarning::NoModifiedDate { path, error }),
        }
    }

    moves.sort_by(|a, b| {
        a.folder_name
            .cmp(&b.folder_name)
            .then_with(|| a.source.cmp(&b.source))
    });

    Ok(Plan {
        base: dir.to_path_buf(),
        moves,
        warnings,
    })
}

/// Ruta canónica del propio ejecutable, para no moverlo nunca. Se resuelve
/// una vez por escaneo, y solo se canonicaliza un archivo cuando su nombre
/// coincide con el del ejecutable.
struct SelfExe(Option<PathBuf>);

impl SelfExe {
    fn locate() -> Self {
        SelfExe(env::current_exe().and_then(|p| p.canonicalize()).ok())
    }

    fn is(&self, path: &Path) -> bool {
        let Some(exe) = &self.0 else { return false };
        if exe.file_name() != path.file_name() {
            return false;
        }
        path.canonicalize().is_ok_and(|p| &p == exe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_lists_only_loose_files_sorted_by_folder_and_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();
        fs::write(dir.path().join("a.txt"), "").unwrap();
        fs::create_dir(dir.path().join("subcarpeta")).unwrap();
        fs::write(dir.path().join("subcarpeta").join("dentro.txt"), "").unwrap();

        let plan = scan(dir.path()).unwrap();

        let names: Vec<_> = plan
            .moves
            .iter()
            .map(|m| m.source.file_name().unwrap().to_owned())
            .collect();
        assert_eq!(names, ["a.txt", "b.txt"]);
        assert!(plan.warnings.is_empty());
    }
}
