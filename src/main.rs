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

    loop {
        let target_dir = match ask_for_directory() {
            Some(dir) => dir,
            None => {
                println!("👋 ¡Hasta luego!");
                break;
            }
        };

        // Recopilar SOLO archivos sueltos en el directorio (sin recursión)
        let mut entries: Vec<FileEntry> = Vec::new();

        if let Err(e) = collect_files(&target_dir, &mut entries) {
            eprintln!("Error al escanear: {}", e);
            continue;
        }

        if entries.is_empty() {
            println!("\n⚠️  No se encontraron archivos sueltos para organizar en esa ruta.");
        } else {
            print_preview(&entries);
            println!("📊 Se moverán {} archivo(s) en total.\n", entries.len());

            if confirm("¿Está seguro de que desea continuar? (s/n): ") {
                let (moved, errors) = execute_moves(&target_dir, &entries);

                println!("\n✅ Resultado:");
                println!("   Archivos movidos:  {moved}");
                println!("   Errores:           {errors}");
            } else {
                println!("❌ Operación cancelada. No se movió ningún archivo.");
            }
        }

        println!("\n─────────────────────────────────────────────");
        if !confirm("¿Desea organizar otra carpeta? (s/n, o presione 'q' para salir): ") {
            println!("👋 ¡Hasta luego!");
            break;
        }
        println!();
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
        let datetime: DateTime<Local> = modified.into();
        let folder_name = datetime.format("Y%Y-M%m").to_string();

        entries.push(FileEntry {
            source: path,
            folder_name,
        });
    }
    Ok(())
}

// ─────────────────────── EJECUTAR MOVIMIENTOS ───────────────

fn execute_moves(base_dir: &Path, entries: &[FileEntry]) -> (u32, u32) {
    let mut moved = 0u32;
    let mut errors = 0u32;

    for entry in entries {
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
            eprintln!("⏭️  Archivo ya no existe: {}", entry.source.display());
            errors += 1;
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

        match move_file(&entry.source, &dest_path) {
            Ok(()) => {
                println!("✔ {} → {}", entry.source.display(), dest_path.display());
                moved += 1;
            }
            Err(e) => {
                eprintln!("✘ Error moviendo '{}': {}", entry.source.display(), e);
                errors += 1;
            }
        }
    }

    (moved, errors)
}

fn move_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    fs::rename(source, dest).or_else(|_| -> std::io::Result<()> {
        fs::copy(source, dest)?;
        fs::remove_file(source)?;
        Ok(())
    })
}

// ─────────────────────── UI HELPERS ─────────────────────────

fn print_banner() {
    println!("╔══════════════════════════════════════════════╗");
    println!("║       📂 File Organizer by Date v0.4.0      ║");
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

fn print_preview(entries: &[FileEntry]) {
    println!("\n🔍 Vista previa:");
    println!("─────────────────────────────────────────────");

    if entries.len() <= 20 {
        for entry in entries {
            let name = entry.source.file_name().unwrap_or_default().to_string_lossy();
            println!("   {} → {}/", name, entry.folder_name);
        }
    } else {
        for entry in entries.iter().take(10) {
            let name = entry.source.file_name().unwrap_or_default().to_string_lossy();
            println!("   {} → {}/", name, entry.folder_name);
        }
        println!("   ... y {} archivos más ...", entries.len() - 15);
        for entry in entries.iter().skip(entries.len() - 5) {
            let name = entry.source.file_name().unwrap_or_default().to_string_lossy();
            println!("   {} → {}/", name, entry.folder_name);
        }
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