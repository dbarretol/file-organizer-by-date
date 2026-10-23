//! Operaciones de disco delicadas: mover sin sobrescribir nunca, resistir
//! condiciones de carrera (TOCTOU) y conservar el mtime en el fallback.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Máximo sufijo numérico (`_1`, `_2`…) que se prueba al resolver un conflicto.
const MAX_CONFLICT_SUFFIX: u32 = 10_000;

#[derive(Debug)]
pub(crate) enum MoveError {
    /// El destino ya existe: move_file nunca sobrescribe. El llamador debe
    /// resolver otro nombre y reintentar.
    DestinationExists,
    /// La copia al destino se completó, pero no se pudo borrar el original:
    /// el archivo queda duplicado (origen y destino).
    CopiedButOriginalRemains(io::Error),
    /// No se pudo mover: fallaron tanto rename como copy.
    Failed { rename: io::Error, copy: io::Error },
}

pub(crate) fn move_file(source: &Path, dest: &Path) -> Result<(), MoveError> {
    // fs::rename sobrescribe el destino en silencio en TODAS las plataformas
    // (en Windows usa MoveFileExW con MOVEFILE_REPLACE_EXISTING), así que
    // primero se reclama el nombre de forma atómica con create_new; rename
    // después solo sustituye ese placeholder, que ya es nuestro. Si el
    // placeholder falla por otra razón (p. ej. permisos), rename/copy
    // fallarán igual y reportarán el error real.
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

    // El placeholder sigue en dest (rename no ocurrió) y es nuestro:
    // copy_then_remove lo rellena o lo retira si la copia falla.
    // Si este remove falla (p. ej. un antivirus reteniendo el handle),
    // copy_then_remove verá AlreadyExists y el reintento usará otro nombre,
    // dejando atrás este placeholder vacío de 0 bytes. Riesgo asumido: es
    // inofensivo y la alternativa (abortar el movimiento) sería peor.
    let _ = fs::remove_file(dest);
    copy_then_remove(source, dest, rename_err)
}

/// Fallback cuando rename no es posible (p. ej. destino en otro volumen).
/// Abre el destino con create_new para no sobrescribir jamás un archivo
/// creado por otro proceso entre la comprobación de conflictos y este punto.
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
        // Preservar lo que fs::rename habría mantenido: sin esto, el archivo
        // copiado quedaría con mtime = ahora, corrompiendo justo el dato por
        // el que esta herramienta organiza.
        dest_file.set_modified(metadata.modified()?)?;
        dest_file.set_permissions(metadata.permissions())?;
        Ok::<(), io::Error>(())
    })();
    drop(dest_file);

    match copy_result {
        Ok(_) => fs::remove_file(source).map_err(MoveError::CopiedButOriginalRemains),
        Err(copy_err) => {
            // No dejar un destino a medio copiar
            let _ = fs::remove_file(dest);
            Err(MoveError::Failed {
                rename: rename_err,
                copy: copy_err,
            })
        }
    }
}

/// Primer nombre libre `<stem>_<n><.ext>` dentro de `dest_dir`.
/// Trabaja con `OsStr` de principio a fin: un nombre que no es UTF-8 válido
/// conserva sus bytes originales en lugar de convertirse en `�`.
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

        // Archivo sin extensión
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
        let name = OsStr::from_bytes(b"\xffdatos.txt");

        let resolved = resolve_conflict(dir.path(), name).unwrap();
        let expected = OsString::from_vec(b"\xffdatos_1.txt".to_vec());
        assert_eq!(resolved.file_name().unwrap(), expected);
    }

    #[cfg(windows)]
    #[test]
    fn resolve_conflict_preserves_non_utf16_names() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let dir = tempfile::tempdir().unwrap();
        // Surrogate aislado: válido en NTFS, no representable en UTF-8
        let wide = |s: &str| {
            let mut v = vec![0xD800u16];
            v.extend(s.encode_utf16());
            OsString::from_wide(&v)
        };

        let resolved = resolve_conflict(dir.path(), &wide("datos.txt")).unwrap();
        assert_eq!(resolved.file_name().unwrap(), wide("datos_1.txt"));
    }

    #[test]
    fn move_file_moves_when_dest_is_free() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let dest = dir.path().join("b.txt");
        fs::write(&source, "contenido").unwrap();

        assert!(move_file(&source, &dest).is_ok());
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "contenido");
    }

    #[test]
    fn move_file_never_overwrites_existing_dest() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let dest = dir.path().join("b.txt");
        fs::write(&source, "nuevo").unwrap();
        fs::write(&dest, "original intacto").unwrap();

        // Simula el TOCTOU: el destino apareció después de la comprobación
        // de conflictos. move_file debe negarse a sobrescribir.
        assert!(matches!(
            move_file(&source, &dest),
            Err(MoveError::DestinationExists)
        ));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "original intacto");
        assert_eq!(fs::read_to_string(&source).unwrap(), "nuevo");
    }

    #[test]
    fn copy_fallback_preserves_source_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let dest = dir.path().join("b.txt");
        fs::write(&source, "contenido").unwrap();

        // mtime antiguo bien distinto de "ahora" (2001-09-09)
        let old_mtime =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_modified(old_mtime)
            .unwrap();

        // En Windows esto funciona porque set_modified explícito suprime la
        // actualización implícita del last-write al cerrar el handle; si
        // copy_then_remove pasara a fijar el mtime por ruta tras cerrar,
        // este test detectaría la regresión.
        let rename_err = io::Error::other("simulado");
        assert!(copy_then_remove(&source, &dest, rename_err).is_ok());
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "contenido");
        assert_eq!(fs::metadata(&dest).unwrap().modified().unwrap(), old_mtime);
    }

    #[test]
    fn copy_fallback_never_overwrites_existing_dest() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let dest = dir.path().join("b.txt");
        fs::write(&source, "nuevo").unwrap();
        fs::write(&dest, "original intacto").unwrap();

        let rename_err = io::Error::other("simulado");
        assert!(matches!(
            copy_then_remove(&source, &dest, rename_err),
            Err(MoveError::DestinationExists)
        ));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "original intacto");
    }
}
