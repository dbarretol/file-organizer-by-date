use chrono::{DateTime, Local};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // Directorio de trabajo: argumento o directorio actual
    let target_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("No se pudo obtener el directorio actual"));

    if !target_dir.is_dir() {
        eprintln!(
            "Error: '{}' no es un directorio válido.",
            target_dir.display()
        );
        std::process::exit(1);
    }

    println!("📂 Organizando archivos en: {}", target_dir.display());

    let mut moved = 0u32;
    let mut errors = 0u32;
    let mut skipped = 0u32;

    // Recorrer recursivamente todos los archivos
    if let Err(e) = walk_and_organize(
        &target_dir,
        &target_dir,
        &mut moved,
        &mut errors,
        &mut skipped,
    ) {
        eprintln!("Error durante el recorrido: {}", e);
    }

    println!("\n✅ Resultado:");
    println!("   Archivos movidos:  {moved}");
    println!("   Archivos omitidos: {skipped}");
    println!("   Errores:           {errors}");
}

fn walk_and_organize(
    base_dir: &Path,
    current_dir: &Path,
    moved: &mut u32,
    errors: &mut u32,
    skipped: &mut u32,
) -> std::io::Result<()> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Si es un directorio con formato "YXXXX-MXX", lo saltamos para no
            // reorganizar carpetas ya creadas por este programa.
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
            if is_organized_folder(&dir_name) {
                *skipped += 1;
                println!("⏭️  Omitiendo carpeta ya organizada: {}", dir_name);
                continue;
            }
            // Recursión en subdirectorios normales
            walk_and_organize(base_dir, &path, moved, errors, skipped)?;
        } else if path.is_file() {
            // No mover el propio ejecutable
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
    // Obtener fecha de modificación
    let metadata = fs::metadata(file_path)?;
    let modified = metadata.modified()?;
    let datetime: DateTime<Local> = modified.into();

    // Crear nombre de carpeta: "Y2024-M03"
    let folder_name = datetime.format("Y%Y-M%m").to_string();
    let dest_dir = base_dir.join(&folder_name);

    // Crear directorio si no existe
    if !dest_dir.exists() {
        fs::create_dir_all(&dest_dir)?;
        println!("📁 Carpeta creada: {}", folder_name);
    }

    // Manejar conflictos de nombre
    let file_name = file_path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Sin nombre de archivo"))?;

    let mut dest_path = dest_dir.join(file_name);

    // Si ya existe un archivo con el mismo nombre, añadir sufijo numérico
    if dest_path.exists() {
        dest_path = resolve_conflict(&dest_dir, file_name)?;
        println!(
            "⚠️  Conflicto de nombre, renombrando a: {}",
            dest_path.file_name().unwrap().to_string_lossy()
        );
    }

    // Mover archivo
    // ✅ DESPUÉS (corregido)
    fs::rename(file_path, &dest_path).or_else(|_| -> std::io::Result<()> {
        fs::copy(file_path, &dest_path)?;
        fs::remove_file(file_path)?;
        Ok(())
    })?;

    Ok(dest_path)
}

/// Resuelve conflictos de nombre añadiendo un sufijo numérico: archivo_1.txt, archivo_2.txt...
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

/// Verifica si un nombre de carpeta tiene formato "YXXXX-MXX"
fn is_organized_folder(name: &str) -> bool {
    // Patrón: Y seguido de 4 dígitos, guion, M seguido de 2 dígitos
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

/// Evita mover el propio ejecutable
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
