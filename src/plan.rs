use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::grouping::{folder_for, is_date_folder};
use crate::options::{DateSource, Options};

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
/// (Carpetas no recorridas, archivos filtrados y el propio ejecutable se
/// omiten en silencio.)
#[derive(Debug)]
pub enum ScanWarning {
    UnreadableEntry(io::Error),
    /// Solo en modo recursivo: una subcarpeta que no se pudo listar.
    UnreadableDir {
        path: PathBuf,
        error: io::Error,
    },
    /// Decisión deliberada: los enlaces simbólicos (y en Windows también las
    /// junctions) se omiten. is_file() y metadata() siguen el enlace, así que
    /// se clasificaría por la fecha del destino, moverlo podría romper
    /// enlaces relativos y recorrerlo podría entrar en un ciclo.
    Symlink(PathBuf),
    NoDate {
        path: PathBuf,
        source: DateSource,
        error: io::Error,
    },
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }
}

/// Lee los archivos sueltos del directorio indicado; con
/// `options.recursive`, también los de sus subcarpetas. Un error al listar
/// `dir` aborta; los errores por archivo o subcarpeta se registran como
/// avisos.
pub fn scan(dir: &Path, options: &Options) -> io::Result<Plan> {
    let mut scanner = Scanner {
        options,
        self_exe: SelfExe::locate(),
        moves: Vec::new(),
        warnings: Vec::new(),
    };
    scanner.visit(dir)?;

    let Scanner {
        mut moves,
        warnings,
        ..
    } = scanner;
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

struct Scanner<'a> {
    options: &'a Options,
    self_exe: SelfExe,
    moves: Vec<PlannedMove>,
    warnings: Vec<ScanWarning>,
}

impl Scanner<'_> {
    fn visit(&mut self, dir: &Path) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    self.warnings.push(ScanWarning::UnreadableEntry(e));
                    continue;
                }
            };
            let path = entry.path();

            if path.is_symlink() {
                self.warnings.push(ScanWarning::Symlink(path));
                continue;
            }

            if path.is_dir() {
                if self.should_descend(&path) {
                    if let Err(error) = self.visit(&path) {
                        self.warnings
                            .push(ScanWarning::UnreadableDir { path, error });
                    }
                }
                continue;
            }

            if path.is_file() && self.wants(&path) {
                self.plan_move(path);
            }
        }
        Ok(())
    }

    fn should_descend(&self, dir: &Path) -> bool {
        self.options.recursive
            && !dir
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(is_date_folder)
    }

    fn wants(&self, file: &Path) -> bool {
        let passes_filter = self
            .options
            .extensions
            .as_ref()
            .is_none_or(|filter| filter.matches(file));
        passes_filter && !self.self_exe.is(file)
    }

    fn plan_move(&mut self, path: PathBuf) {
        let source = self.options.date_source;
        match fs::metadata(&path).and_then(|m| source.read(&m)) {
            Ok(date) => self.moves.push(PlannedMove {
                folder_name: folder_for(date.into(), self.options.granularity),
                source: path,
            }),
            Err(error) => self.warnings.push(ScanWarning::NoDate {
                path,
                source,
                error,
            }),
        }
    }
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
    use crate::filter::ExtensionFilter;
    use crate::grouping::Granularity;

    #[test]
    fn scan_lists_only_loose_files_sorted_by_folder_and_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();
        fs::write(dir.path().join("a.txt"), "").unwrap();
        fs::create_dir(dir.path().join("subcarpeta")).unwrap();
        fs::write(dir.path().join("subcarpeta").join("dentro.txt"), "").unwrap();

        let plan = scan(dir.path(), &Options::default()).unwrap();

        let names: Vec<_> = plan
            .moves
            .iter()
            .map(|m| m.source.file_name().unwrap().to_owned())
            .collect();
        assert_eq!(names, ["a.txt", "b.txt"]);
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn scan_uses_the_requested_granularity() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "").unwrap();

        let options = Options {
            granularity: Granularity::Day,
            ..Options::default()
        };
        let plan = scan(dir.path(), &options).unwrap();

        let mtime = fs::metadata(dir.path().join("a.txt"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(
            plan.moves[0].folder_name,
            folder_for(mtime.into(), Granularity::Day)
        );
    }

    #[test]
    fn scan_filters_by_extension() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["foto.JPG", "nota.txt", "clip.png"] {
            fs::write(dir.path().join(name), "").unwrap();
        }

        let options = Options {
            extensions: Some(ExtensionFilter::new(["jpg", "png"])),
            ..Options::default()
        };
        let plan = scan(dir.path(), &options).unwrap();

        assert_eq!(names(&plan), ["clip.png", "foto.JPG"]);
    }

    #[test]
    fn recursive_scan_descends_but_skips_date_folders() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        fs::create_dir_all(base.join("fotos").join("viaje")).unwrap();
        fs::create_dir(base.join("Y2020-M05")).unwrap();
        fs::write(base.join("raiz.txt"), "").unwrap();
        fs::write(base.join("fotos").join("a.jpg"), "").unwrap();
        fs::write(base.join("fotos").join("viaje").join("b.jpg"), "").unwrap();
        fs::write(base.join("Y2020-M05").join("ya_organizado.txt"), "").unwrap();

        let flat = scan(base, &Options::default()).unwrap();
        assert_eq!(names(&flat), ["raiz.txt"]);

        let options = Options {
            recursive: true,
            ..Options::default()
        };
        let deep = scan(base, &options).unwrap();
        assert_eq!(names(&deep), ["a.jpg", "b.jpg", "raiz.txt"]);
    }

    /// Nombres de archivo del plan, ordenados alfabéticamente.
    fn names(plan: &Plan) -> Vec<String> {
        let mut names: Vec<String> = plan
            .moves
            .iter()
            .map(|m| m.source.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}
