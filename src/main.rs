use chrono::{DateTime, Local};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

// ─────────────────────────── MAIN ───────────────────────────

fn main() {
    print_banner();

    loop {
        // 1. Pedir ruta al usuario
        let target_dir = match ask_for_directory() {
            Some(dir) => dir,
            None => {
                println!("👋 ¡Hasta luego!");
                break;
            }
        };

        // 2. Dry-run: contar archivos que se moverían
        let (file_count, dry_run_preview) = dry_run_count(&target_dir);

        if file_count == 0 {
            println!("\n⚠️  No se encontraron archivos para organizar en esa ruta.");
        } else {
            // 3. Mostrar preview y pedir confirmación
            println!("\n{}", dry_run_preview);
            println!("📊 Se moverán {} archivo(s) en total.\n", file_count);

            if confirm("¿Está seguro de que desea continuar? (s/n): ") {
                // 4. Ejecutar la organización real
                let mut moved = 0u32;
                let mut errors = 0u32;
                let mut skipped = 0u32;

                println!();
                if let Err(e) = walk_and_organize(
                    &target_dir,
                    &target_dir,
                    &mut moved,
                    &mut errors,
                    &mut skipped,
                    false, // dry_run = false
                ) {
                    eprintln!("Error durante el recorrido: {}", e);
                }

                println!("\n✅ Resultado:");
                println!("   Archivos movidos:  {moved}");
                println!("   Archivos omitidos: {skipped}");
                println!("   Errores:           {errors}");
            } else {
                println!("❌ Operación cancelada. No se movió ningún archivo.");
            }
        }

        // 5. Preguntar si quiere organizar otra carpeta
        println!("\n─────────────────────────────────────────────");
        if !confirm("¿Desea organizar otra carpeta? (s/n, o presione 'q' para salir): ") {
            println!("👋 ¡Hasta luego!");
            break;
        }
        println!();
    }
}

// ─────────────────────────── UI HELPERS ───────────────────────────

fn print_banner() {
    println!("╔══════════════════════════════════════════════╗");
    println!("║       📂 File Organizer by Date v0.2.0      ║");
    println!("║  Organiza archivos por fecha de modificación ║");
    println!("╚══════════════════════════════════════════════╝");
    println!();
}

/// Pide al usuario una ruta. Devuelve None si el usuario escribe 'q'.
fn ask_for_directory() -> Option<PathBuf> {
    loop {
        print!("📁 Ingrese la ruta de la carpeta a organizar (o 'q' para salir): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.eq_ignore_ascii_case("q") {
            return None;
        }

        if input.is_empty() {
            println!("⚠️  La ruta no puede estar vacía. Intente de nuevo.\n");
            continue;
        }

        // Limpiar comillas que el usuario pueda pegar desde el explorador
        let cleaned = input.trim_matches('"').trim_matches('\'');
        let path = PathBuf::from(cleaned);

        if !path.exists() {
            println!(
                "⚠️  La ruta '{}' no existe. Intente de nuevo.\n",
                path.display()
            );
            continue;
        }

        if !path.is_dir() {
            println!(
                "⚠️  '{}' no es un directorio. Intente de nuevo.\n",
                path.display()
            );
            continue;
        }

        return Some(path);
    }
}

/// Pregunta sí/no al usuario. Acepta s, si, y, yes, q (como no).
fn confirm(prompt: &str) -> bool {
    loop {
        print!("{}", prompt);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        match input.as_str() {
            "s" | "si" | "sí" | "y" | "yes" => return true,
            "n" | "no" | "q" | "quit" | "exit" => return false,
            _ => println!("   Por favor, responda 's' (sí) o 'n' (no)."),
        }
    }
}

// ─────────────────────────── DRY RUN ───────────────────────────

/// Cuenta los archivos que se moverían y genera un preview.
fn dry_run_count(base_dir: &Path) -> (u32, String) {
    let mut count = 0u32;
    let mut skipped = 0u32;
    let mut errors = 0u32;
    let mut preview_lines: Vec<String> = Vec::new();

    // Recopilar usando el mismo walk, pero en modo dry_run
    let _ = walk_and_collect_preview(
        base_dir,
        base_dir,
        &mut count,
        &mut skipped,
        &mut errors,
        &mut preview_lines,
    );

    let mut preview = String::from("🔍 Vista previa (dry-run):\n");
    preview.push_str("─────────────────────────────────────────────\n");

    if preview_lines.len() <= 20 {
        for line in &preview_lines {
            preview.push_str(line);
            preview.push('\n');
        }
    } else {
        // Mostrar primeros 10 y últimos 5
        for line in preview_lines.iter().take(10) {
            preview.push_str(line);
            preview.push('\n');
        }
        preview.push_str(&format!(
            "   ... y {} archivos más ...\n",
            preview_lines.len() - 15
        ));
        for line in preview_lines.iter().skip(preview_lines.len() - 5) {
            preview.push_str(line);
            preview.push('\n');
        }
    }

    if skipped > 0 {
        preview.push_str(&format!(
            "⏭️  {} carpeta(s) ya organizada(s) serán omitidas.\n",
            skipped
        ));
    }

    preview.push_str("─────────────────────────────────────────────");

    (count, preview)
}

fn walk_and_collect_preview(
    base_dir: &Path,
    current_dir: &Path,
    count: &mut u32,
    skipped: &mut u32,
    errors: &mut u32,
    lines: &mut Vec<String>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
            if is_organized_folder(&dir_name) {
                *skipped += 1;
                continue;
            }
            walk_and_collect_preview(base_dir, &path, count, skipped, errors, lines)?;
        } else if path.is_file() {
            if is_self_executable(&path) {
                continue;
            }

            match get_destination_info(base_dir, &path) {
                Ok(folder_name) => {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                    lines.push(format!("   {} → {}/", file_name, folder_name));
                    *count += 1;
                }
                Err(e) => {
                    lines.push(format!("   ✘ {} (error: {})", path.display(), e));
                    *errors += 1;
                }
            }
        }
    }
    Ok(())
}

/// Obtiene el nombre de carpeta destino sin mover el archivo.
fn get_destination_info(base_dir: &Path, file_path: &Path) -> std::io::Result<String> {
    let metadata = fs::metadata(file_path)?;
    let modified = metadata.modified()?;
    let datetime: DateTime<Local> = modified.into();
    let folder_name = datetime.format("Y%Y-M%m").to_string();
    let _ = base_dir; // se usa para mantener la firma consistente
    Ok(folder_name)
}

// ─────────────────────────── ORGANIZE ───────────────────────────

fn walk_and_organize(
    base_dir: &Path,
    current_dir: &Path,
    moved: &mut u32,
    errors: &mut u32,
    skipped: &mut u32,
    _dry_run: bool,
) -> std::io::Result<()> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
            if is_organized_folder(&dir_name) {
                *skipped += 1;
                println!("⏭️  Omitiendo carpeta ya organizada: {}", dir_name);
                continue;
            }
            walk_and_organize(base_dir, &path, moved, errors, skipped, _dry_run)?;
        } else if path.is_file() {
            if is_self_executable(&path) {
                continue;
            }

            match organize_file(base_dir, &path) {
                Ok(dest) => {
                    println!("✔ {} → {}", path.display(), dest.display());
                    *moved += 1;
                }
                Err(e) => {
                    eprintln!("✘ Error moviendo '{}': {}", path.display(), e);
                    *errors += 1;
                }
            }
        }
    }
    Ok(())
}

fn organize_file(base_dir: &Path, file_path: &Path) -> std::io::Result<PathBuf> {
    let metadata = fs::metadata(file_path)?;
    let modified = metadata.modified()?;
    let datetime: DateTime<Local> = modified.into();

    let folder_name = datetime.format("Y%Y-M%m").to_string();
    let dest_dir = base_dir.join(&folder_name);

    if !dest_dir.exists() {
        fs::create_dir_all(&dest_dir)?;
        println!("📁 Carpeta creada: {}", folder_name);
    }

    let file_name = file_path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Sin nombre de archivo"))?;

    let mut dest_path = dest_dir.join(file_name);

    if dest_path.exists() {
        dest_path = resolve_conflict(&dest_dir, file_name)?;
        println!(
            "⚠️  Conflicto de nombre, renombrando a: {}",
            dest_path.file_name().unwrap().to_string_lossy()
        );
    }

    fs::rename(file_path, &dest_path).or_else(|_| -> std::io::Result<()> {
        fs::copy(file_path, &dest_path)?;
        fs::remove_file(file_path)?;
        Ok(())
    })?;

    Ok(dest_path)
}

// ─────────────────────────── UTILS ───────────────────────────

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

fn is_organized_folder(name: &str) -> bool {
    if name.len() != 8 {
        return false;
    }
    let bytes = name.as_bytes();
    bytes[0] == b'Y'
        && bytes[1..5].iter().all(|b| b.is_ascii_digit())
        && bytes[5] == b'-'
        && bytes[6] == b'M'
        && bytes[7..].iter().all(|b| b.is_ascii_digit())
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
