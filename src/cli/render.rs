//! Todo el texto que la CLI muestra a partir de los datos del núcleo.

use std::io;
use std::path::Path;

use file_organizer_by_date::{Event, Failure, Plan, ScanWarning, Summary};

/// Máximo de archivos listados por carpeta en la vista previa.
const MAX_PREVIEW_PER_GROUP: usize = 10;

const SEPARATOR: &str = "─────────────────────────────────────────────";

pub fn banner() {
    // La versión sale de Cargo.toml para no desincronizarse, y el marco se
    // dimensiona según el texto para que cualquier longitud (0.4.0, 0.10.0…)
    // quede centrada.
    let title = format!("📂 File Organizer by Date v{}", env!("CARGO_PKG_VERSION"));
    let subtitle = "Organiza archivos por fecha de modificación";

    // Columnas de terminal: el emoji cuenta como 1 char pero ocupa 2 columnas
    let title_cols = title.chars().count() + 1;
    let subtitle_cols = subtitle.chars().count();
    let inner = title_cols.max(subtitle_cols) + 4;

    let centered = |text: &str, cols: usize| {
        let left = (inner - cols) / 2;
        format!(
            "{}{}{}",
            " ".repeat(left),
            text,
            " ".repeat(inner - cols - left)
        )
    };

    println!("╔{}╗", "═".repeat(inner));
    println!("║{}║", centered(&title, title_cols));
    println!("║{}║", centered(subtitle, subtitle_cols));
    println!("╚{}╝", "═".repeat(inner));
    println!();
}

pub fn quote_hint() {
    eprintln!("Si la ruta contiene espacios, escríbala entre comillas:");
    eprintln!("     file-organizer-by-date \"C:\\Mi Carpeta\"");
}

pub fn invalid_dir(path: &Path) {
    eprintln!("✘ '{}' no existe o no es un directorio.", path.display());
}

pub fn scan_error(error: &io::Error) {
    eprintln!("Error al escanear: {error}");
}

pub fn warnings(warnings: &[ScanWarning]) {
    for warning in warnings {
        match warning {
            ScanWarning::UnreadableEntry(e) => {
                eprintln!("⏭️  Se omite una entrada ilegible: {e}")
            }
            ScanWarning::Symlink(path) => {
                println!("⏭️  Se omite enlace simbólico: {}", path.display())
            }
            ScanWarning::NoModifiedDate { path, error } => eprintln!(
                "⏭️  Se omite '{}' (no se pudo leer su fecha de modificación): {}",
                path.display(),
                error
            ),
        }
    }
}

pub fn nothing_to_do() {
    println!("\n⚠️  No se encontraron archivos sueltos para organizar en esa ruta.");
}

/// Muestra los movimientos agrupados por carpeta destino, con un conteo por
/// grupo y listando como máximo `MAX_PREVIEW_PER_GROUP` archivos de cada uno.
/// Se apoya en que `Plan::moves` viene ordenado por `folder_name`.
pub fn preview(plan: &Plan) {
    let moves = &plan.moves;

    println!("\n🔍 Vista previa:");
    println!("{SEPARATOR}");

    let mut start = 0;
    while start < moves.len() {
        let folder = &moves[start].folder_name;
        let count = moves[start..]
            .iter()
            .take_while(|m| &m.folder_name == folder)
            .count();

        println!("   📁 {}/  ({} archivo(s))", folder, count);
        for planned in &moves[start..start + count.min(MAX_PREVIEW_PER_GROUP)] {
            let name = planned
                .source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            println!("      {}", name);
        }
        if count > MAX_PREVIEW_PER_GROUP {
            println!("      ... y {} más", count - MAX_PREVIEW_PER_GROUP);
        }

        start += count;
    }

    println!("{SEPARATOR}");
    println!("📊 Se moverán {} archivo(s) en total.\n", moves.len());
}

pub fn cancelled() {
    println!("❌ Operación cancelada. No se movió ningún archivo.");
}

pub fn event(event: Event) {
    match event {
        Event::FolderCreated { folder_name } => println!("📁 Carpeta creada: {folder_name}"),
        Event::Conflict {
            original,
            renamed_to,
            late: false,
        } => println!(
            "⚠️  Conflicto: '{}' ya existe → renombrando a '{}'",
            original.to_string_lossy(),
            renamed_to.to_string_lossy()
        ),
        Event::Conflict {
            original,
            renamed_to,
            late: true,
        } => println!(
            "⚠️  Conflicto: '{}' apareció en el destino → reintentando como '{}'",
            original.to_string_lossy(),
            renamed_to.to_string_lossy()
        ),
        Event::Moved { from, to } => println!("✔ {} → {}", from.display(), to.display()),
        Event::SourceMissing { path } => println!("⏭️  Archivo ya no existe: {}", path.display()),
        Event::Failed { source, failure } => failed(&source, &failure),
    }
}

fn failed(source: &Path, failure: &Failure) {
    let source = source.display();
    match failure {
        Failure::DestNotADirectory { dest_dir } => eprintln!(
            "✘ '{}' ya existe pero no es una carpeta; se omite '{}'",
            dest_dir.display(),
            source
        ),
        Failure::CreateDir { dest_dir, error } => {
            eprintln!(
                "✘ Error creando carpeta '{}': {}",
                dest_dir.display(),
                error
            )
        }
        Failure::NoFileName => {
            eprintln!("✘ Se omite '{source}': la ruta no tiene nombre de archivo")
        }
        Failure::ResolveConflict(error) => eprintln!("✘ Error resolviendo conflicto: {error}"),
        Failure::DestinationBusy { attempts } => eprintln!(
            "✘ No se pudo mover '{source}': el destino sigue ocupado tras {attempts} intentos"
        ),
        Failure::CopiedButOriginalRemains { dest, error } => eprintln!(
            "⚠️  '{}' se copió a '{}' pero no se pudo borrar el original (queda duplicado): {}",
            source,
            dest.display(),
            error
        ),
        Failure::Move { rename, copy } => {
            eprintln!("✘ Error moviendo '{source}': {copy} (rename falló con: {rename})")
        }
    }
}

pub fn summary(summary: &Summary) {
    println!("\n✅ Resultado:");
    println!("   Archivos movidos:  {}", summary.moved);
    println!("   Omitidos:          {}", summary.skipped);
    println!("   Errores:           {}", summary.errors);
}

pub fn separator() {
    println!("\n{SEPARATOR}");
}

pub fn goodbye() {
    println!("👋 ¡Hasta luego!");
}
