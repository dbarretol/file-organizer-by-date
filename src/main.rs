use chrono::{DateTime, Local};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

// ─────────────────────── ESTRUCTURAS ────────────────────────

struct FileEntry {
    source: PathBuf,
    folder_name: String,
}

// ─────────────────────────── MAIN ───────────────────────────

fn main() {
    print_banner();

    // Modo scriptable: la ruta puede pasarse como argumento
    // (p. ej. `file-organizer-by-date C:\Descargas`); se procesa una sola
    // carpeta y se termina, sin el bucle interactivo.
    if let Some(arg) = env::args_os().nth(1) {
        let path = PathBuf::from(arg);
        if !path.is_dir() {
            eprintln!("✘ '{}' no existe o no es un directorio.", path.display());
            std::process::exit(1);
        }
        organize_directory(&path);
        return;
    }

    loop {
        let target_dir = match ask_for_directory() {
            Some(dir) => dir,
            None => {
                println!("👋 ¡Hasta luego!");
                break;
            }
        };

        organize_directory(&target_dir);

        println!("\n─────────────────────────────────────────────");
        if !confirm("¿Desea organizar otra carpeta? (s/n, o presione 'q' para salir): ") {
            println!("👋 ¡Hasta luego!");
            break;
        }
        println!();
    }
}

/// Flujo completo sobre una carpeta: escanear, previsualizar, confirmar, mover.
fn organize_directory(target_dir: &Path) {
    // Recopilar SOLO archivos sueltos en el directorio (sin recursión)
    let mut entries: Vec<FileEntry> = Vec::new();

    if let Err(e) = collect_files(target_dir, &mut entries) {
        eprintln!("Error al escanear: {}", e);
        return;
    }

    // Orden estable por carpeta destino y nombre: preview agrupado y
    // movimientos en orden predecible
    entries.sort_by(|a, b| {
        a.folder_name
            .cmp(&b.folder_name)
            .then_with(|| a.source.cmp(&b.source))
    });

    if entries.is_empty() {
        println!("\n⚠️  No se encontraron archivos sueltos para organizar en esa ruta.");
        return;
    }

    print_preview(&entries);
    println!("📊 Se moverán {} archivo(s) en total.\n", entries.len());

    if confirm("¿Está seguro de que desea continuar? (s/n): ") {
        let (moved, skipped, errors) = execute_moves(target_dir, &entries);

        println!("\n✅ Resultado:");
        println!("   Archivos movidos:  {moved}");
        println!("   Omitidos:          {skipped}");
        println!("   Errores:           {errors}");
    } else {
        println!("❌ Operación cancelada. No se movió ningún archivo.");
    }
}

// ─────────────────────── RECOPILAR ARCHIVOS ─────────────────

/// Lee SOLO los archivos sueltos del directorio indicado (sin recursión).
fn collect_files(
    dir: &Path,
    entries: &mut Vec<FileEntry>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        // Errores por archivo: saltar con aviso en lugar de abortar el escaneo
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                eprintln!("⏭️  Se omite una entrada ilegible: {}", e);
                continue;
            }
        };
        let path = entry.path();

        // Decisión deliberada: los enlaces simbólicos se omiten. is_file() y
        // metadata() siguen el enlace, así que se clasificaría por la fecha
        // del destino y moverlo podría romper enlaces relativos.
        if path.is_symlink() {
            println!("⏭️  Se omite enlace simbólico: {}", path.display());
            continue;
        }

        // Solo archivos, ignorar carpetas por completo
        if !path.is_file() {
            continue;
        }

        // No mover el propio ejecutable
        if is_self_executable(&path) {
            continue;
        }

        let modified = match fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(modified) => modified,
            Err(e) => {
                eprintln!(
                    "⏭️  Se omite '{}' (no se pudo leer su fecha de modificación): {}",
                    path.display(),
                    e
                );
                continue;
            }
        };
        entries.push(FileEntry {
            source: path,
            folder_name: folder_for(modified.into()),
        });
    }
    Ok(())
}

/// Nombre de la carpeta destino para una fecha de modificación dada.
fn folder_for(datetime: DateTime<Local>) -> String {
    datetime.format("Y%Y-M%m").to_string()
}

// ─────────────────────── EJECUTAR MOVIMIENTOS ───────────────

/// Devuelve (movidos, omitidos, errores).
fn execute_moves(base_dir: &Path, entries: &[FileEntry]) -> (u32, u32, u32) {
    let mut moved = 0u32;
    let mut skipped = 0u32;
    let mut errors = 0u32;

    'files: for entry in entries {
        let dest_dir = base_dir.join(&entry.folder_name);

        if !dest_dir.is_dir() {
            // La ruta destino puede existir como archivo (p. ej. un archivo
            // llamado "Y2026-M07" sin extensión): reportar en vez de intentar
            // mover dentro de algo que no es una carpeta.
            if dest_dir.exists() {
                eprintln!(
                    "✘ '{}' ya existe pero no es una carpeta; se omite '{}'",
                    dest_dir.display(),
                    entry.source.display()
                );
                errors += 1;
                continue;
            }
            if let Err(e) = fs::create_dir_all(&dest_dir) {
                eprintln!("✘ Error creando carpeta '{}': {}", dest_dir.display(), e);
                errors += 1;
                continue;
            }
            println!("📁 Carpeta creada: {}", entry.folder_name);
        }

        if !entry.source.exists() {
            // Desapareció entre el escaneo y la ejecución: no es un fallo
            // nuestro, se cuenta como omitido
            println!("⏭️  Archivo ya no existe: {}", entry.source.display());
            skipped += 1;
            continue;
        }

        let file_name = match entry.source.file_name() {
            Some(name) => name,
            None => {
                errors += 1;
                continue;
            }
        };

        let mut dest_path = dest_dir.join(file_name);

        // La comprobación con exists() es solo orientativa: move_file nunca
        // sobrescribe, así que si otro proceso crea el destino justo después
        // de comprobar (TOCTOU), se resuelve otro nombre y se reintenta.
        if dest_path.exists() {
            match resolve_conflict(&dest_dir, file_name) {
                Ok(resolved) => {
                    println!(
                        "⚠️  Conflicto: '{}' ya existe → renombrando a '{}'",
                        file_name.to_string_lossy(),
                        resolved.file_name().unwrap().to_string_lossy()
                    );
                    dest_path = resolved;
                }
                Err(e) => {
                    eprintln!("✘ Error resolviendo conflicto: {}", e);
                    errors += 1;
                    continue;
                }
            }
        }

        let mut attempts = 0;
        loop {
            match move_file(&entry.source, &dest_path) {
                Ok(()) => {
                    println!("✔ {} → {}", entry.source.display(), dest_path.display());
                    moved += 1;
                    continue 'files;
                }
                Err(MoveError::DestinationExists) => {
                    attempts += 1;
                    if attempts > 5 {
                        eprintln!(
                            "✘ No se pudo mover '{}': el destino sigue ocupado tras {} intentos",
                            entry.source.display(),
                            attempts
                        );
                        errors += 1;
                        continue 'files;
                    }
                    match resolve_conflict(&dest_dir, file_name) {
                        Ok(resolved) => {
                            println!(
                                "⚠️  Conflicto: '{}' apareció en el destino → reintentando como '{}'",
                                dest_path.file_name().unwrap_or_default().to_string_lossy(),
                                resolved.file_name().unwrap().to_string_lossy()
                            );
                            dest_path = resolved;
                        }
                        Err(e) => {
                            eprintln!("✘ Error resolviendo conflicto: {}", e);
                            errors += 1;
                            continue 'files;
                        }
                    }
                }
                Err(MoveError::CopiedButOriginalRemains(e)) => {
                    eprintln!(
                        "⚠️  '{}' se copió a '{}' pero no se pudo borrar el original (queda duplicado): {}",
                        entry.source.display(),
                        dest_path.display(),
                        e
                    );
                    errors += 1;
                    continue 'files;
                }
                Err(MoveError::Failed { rename, copy }) => {
                    eprintln!(
                        "✘ Error moviendo '{}': {} (rename falló con: {})",
                        entry.source.display(),
                        copy,
                        rename
                    );
                    errors += 1;
                    continue 'files;
                }
            }
        }
    }

    (moved, skipped, errors)
}

enum MoveError {
    /// El destino ya existe: move_file nunca sobrescribe. El llamador debe
    /// resolver otro nombre y reintentar.
    DestinationExists,
    /// La copia al destino se completó, pero no se pudo borrar el original:
    /// el archivo queda duplicado (origen y destino).
    CopiedButOriginalRemains(io::Error),
    /// No se pudo mover: fallaron tanto rename como copy.
    Failed { rename: io::Error, copy: io::Error },
}

fn move_file(source: &Path, dest: &Path) -> Result<(), MoveError> {
    // fs::rename sobrescribe el destino en silencio en TODAS las plataformas
    // (en Windows usa MoveFileExW con MOVEFILE_REPLACE_EXISTING), así que
    // primero se reclama el nombre de forma atómica con create_new; rename
    // después solo sustituye ese placeholder, que ya es nuestro. Si el
    // placeholder falla por otra razón (p. ej. permisos), rename/copy
    // fallarán igual y reportarán el error real.
    match fs::OpenOptions::new().write(true).create_new(true).open(dest) {
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
    let mut dest_file = match fs::OpenOptions::new().write(true).create_new(true).open(dest) {
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

// ─────────────────────── UI HELPERS ─────────────────────────

fn print_banner() {
    // La versión sale de Cargo.toml para no desincronizarse
    println!("╔══════════════════════════════════════════════╗");
    println!(
        "║       📂 File Organizer by Date v{}      ║",
        env!("CARGO_PKG_VERSION")
    );
    println!("║  Organiza archivos por fecha de modificación ║");
    println!("╚══════════════════════════════════════════════╝");
    println!();
}

fn ask_for_directory() -> Option<PathBuf> {
    loop {
        print!("📁 Ingrese la ruta de la carpeta a organizar (o 'q' para salir): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        // EOF (Ctrl+Z / pipe agotado): salir en lugar de reintentar para siempre
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            println!();
            return None;
        }
        let input = input.trim();

        if input.eq_ignore_ascii_case("q") {
            return None;
        }

        if input.is_empty() {
            println!("⚠️  La ruta no puede estar vacía. Intente de nuevo.\n");
            continue;
        }

        let cleaned = input.trim_matches('"').trim_matches('\'');
        let path = PathBuf::from(cleaned);

        if !path.exists() {
            println!("⚠️  La ruta '{}' no existe. Intente de nuevo.\n", path.display());
            continue;
        }

        if !path.is_dir() {
            println!("⚠️  '{}' no es un directorio. Intente de nuevo.\n", path.display());
            continue;
        }

        return Some(path);
    }
}

fn confirm(prompt: &str) -> bool {
    loop {
        print!("{}", prompt);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        // EOF (Ctrl+Z / pipe agotado): tratar como "no" en lugar de reintentar para siempre
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            println!();
            return false;
        }
        let input = input.trim().to_lowercase();

        match input.as_str() {
            "s" | "si" | "sí" | "y" | "yes" => return true,
            "n" | "no" | "q" | "quit" | "exit" => return false,
            _ => println!("   Por favor, responda 's' (sí) o 'n' (no)."),
        }
    }
}

/// Muestra las entradas agrupadas por carpeta destino, con un conteo por
/// grupo y listando como máximo `MAX_PER_GROUP` archivos de cada uno.
/// Asume que `entries` viene ordenado por `folder_name`.
fn print_preview(entries: &[FileEntry]) {
    const MAX_PER_GROUP: usize = 10;

    println!("\n🔍 Vista previa:");
    println!("─────────────────────────────────────────────");

    let mut start = 0;
    while start < entries.len() {
        let folder = &entries[start].folder_name;
        let count = entries[start..]
            .iter()
            .take_while(|e| &e.folder_name == folder)
            .count();

        println!("   📁 {}/  ({} archivo(s))", folder, count);
        for entry in &entries[start..start + count.min(MAX_PER_GROUP)] {
            let name = entry.source.file_name().unwrap_or_default().to_string_lossy();
            println!("      {}", name);
        }
        if count > MAX_PER_GROUP {
            println!("      ... y {} más", count - MAX_PER_GROUP);
        }

        start += count;
    }

    println!("─────────────────────────────────────────────");
}

// ─────────────────────── UTILIDADES ─────────────────────────

fn resolve_conflict(dest_dir: &Path, file_name: &std::ffi::OsStr) -> std::io::Result<PathBuf> {
    let name = file_name.to_string_lossy();
    let path = Path::new(&*name);

    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for i in 1..=10_000 {
        let new_name = format!("{}_{}{}", stem, i, extension);
        let candidate = dest_dir.join(&new_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "No se pudo resolver el conflicto de nombre tras 10000 intentos",
    ))
}

fn is_self_executable(path: &Path) -> bool {
    if let Ok(exe_path) = env::current_exe() {
        if let (Ok(canonical_exe), Ok(canonical_path)) =
            (exe_path.canonicalize(), path.canonicalize())
        {
            return canonical_exe == canonical_path;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn folder_for_formats_year_and_zero_padded_month() {
        let dt = Local.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap();
        assert_eq!(folder_for(dt), "Y2026-M07");

        let dt = Local.with_ymd_and_hms(1999, 12, 31, 23, 59, 59).unwrap();
        assert_eq!(folder_for(dt), "Y1999-M12");
    }

    #[test]
    fn resolve_conflict_picks_first_free_suffix() {
        let dir = temp_dir("conflict");
        fs::write(dir.join("a.txt"), "").unwrap();
        fs::write(dir.join("a_1.txt"), "").unwrap();

        let resolved = resolve_conflict(&dir, std::ffi::OsStr::new("a.txt")).unwrap();
        assert_eq!(resolved.file_name().unwrap(), "a_2.txt");

        // Archivo sin extensión
        fs::write(dir.join("README"), "").unwrap();
        let resolved = resolve_conflict(&dir, std::ffi::OsStr::new("README")).unwrap();
        assert_eq!(resolved.file_name().unwrap(), "README_1");

        let _ = fs::remove_dir_all(&dir);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("fo_move_test_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn move_file_moves_when_dest_is_free() {
        let dir = temp_dir("free");
        let source = dir.join("a.txt");
        let dest = dir.join("b.txt");
        fs::write(&source, "contenido").unwrap();

        assert!(move_file(&source, &dest).is_ok());
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "contenido");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn move_file_never_overwrites_existing_dest() {
        let dir = temp_dir("occupied");
        let source = dir.join("a.txt");
        let dest = dir.join("b.txt");
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

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_fallback_preserves_source_mtime() {
        let dir = temp_dir("mtime");
        let source = dir.join("a.txt");
        let dest = dir.join("b.txt");
        fs::write(&source, "contenido").unwrap();

        // mtime antiguo bien distinto de "ahora" (2001-09-09)
        let old_mtime = std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(1_000_000_000);
        fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_modified(old_mtime)
            .unwrap();

        let rename_err = io::Error::other("simulado");
        assert!(copy_then_remove(&source, &dest, rename_err).is_ok());
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "contenido");
        assert_eq!(fs::metadata(&dest).unwrap().modified().unwrap(), old_mtime);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_fallback_never_overwrites_existing_dest() {
        let dir = temp_dir("fallback");
        let source = dir.join("a.txt");
        let dest = dir.join("b.txt");
        fs::write(&source, "nuevo").unwrap();
        fs::write(&dest, "original intacto").unwrap();

        let rename_err = io::Error::other("simulado");
        assert!(matches!(
            copy_then_remove(&source, &dest, rename_err),
            Err(MoveError::DestinationExists)
        ));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "original intacto");

        let _ = fs::remove_dir_all(&dir);
    }
}