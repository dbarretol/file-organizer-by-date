use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

/// Lo que `execute` comunica mientras trabaja. Son valores *owned* para
/// poder enviarlos por un canal desde un hilo de trabajo (p. ej. a una GUI).
#[derive(Debug)]
pub enum Event {
    FolderCreated {
        folder_name: String,
    },
    /// El nombre ya estaba ocupado en el destino y se usará otro.
    /// `late` indica que el conflicto apareció después de la comprobación
    /// inicial (otro proceso creó el archivo: TOCTOU) y se reintenta.
    Conflict {
        original: OsString,
        renamed_to: OsString,
        late: bool,
    },
    Moved {
        from: PathBuf,
        to: PathBuf,
    },
    /// Desapareció entre el escaneo y la ejecución: no es un fallo nuestro,
    /// se cuenta como omitido.
    SourceMissing {
        path: PathBuf,
    },
    Failed {
        source: PathBuf,
        failure: Failure,
    },
}

/// Por qué no se pudo mover un archivo. Cada una cuenta como error.
#[derive(Debug)]
pub enum Failure {
    /// La ruta destino existe pero no es una carpeta (p. ej. un archivo
    /// llamado "Y2026-M07" sin extensión).
    DestNotADirectory {
        dest_dir: PathBuf,
    },
    CreateDir {
        dest_dir: PathBuf,
        error: io::Error,
    },
    NoFileName,
    ResolveConflict(io::Error),
    /// El destino siguió ocupado tras varios reintentos.
    DestinationBusy {
        attempts: u32,
    },
    /// La copia al destino se completó, pero no se pudo borrar el original:
    /// el archivo queda duplicado (origen y destino).
    CopiedButOriginalRemains {
        dest: PathBuf,
        error: io::Error,
    },
    /// Fallaron tanto rename como copy.
    Move {
        rename: io::Error,
        copy: io::Error,
    },
}
