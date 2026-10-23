//! Todo el texto que la CLI muestra a partir de los datos del núcleo.

use std::io;
use std::path::Path;

use file_organizer_by_date::{
    DateSource, Event, Failure, Journal, Plan, ScanWarning, Summary, JOURNAL_FILE_NAME,
};

/// Máximo de archivos listados por carpeta en la vista previa.
const MAX_PREVIEW_PER_GROUP: usize = 10;

/// Máximo de movimientos listados en la vista previa de --undo.
const MAX_UNDO_PREVIEW: usize = 20;

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

/// Una línea de salida y el canal al que va. Separar el texto de la
/// impresión permite reutilizarlo en el log (`--log`).
pub struct Line {
    pub text: String,
    pub to_stderr: bool,
}

impl Line {
    fn out(text: String) -> Self {
        Line {
            text,
            to_stderr: false,
        }
    }

    fn err(text: String) -> Self {
        Line {
            text,
            to_stderr: true,
        }
    }

    pub fn print(&self) {
        if self.to_stderr {
            eprintln!("{}", self.text);
        } else {
            println!("{}", self.text);
        }
    }
}

pub fn warning_line(warning: &ScanWarning) -> Line {
    match warning {
        ScanWarning::UnreadableEntry(e) => {
            Line::err(format!("⏭️  Se omite una entrada ilegible: {e}"))
        }
        ScanWarning::Symlink(path) => {
            Line::out(format!("⏭️  Se omite enlace simbólico: {}", path.display()))
        }
        ScanWarning::UnreadableDir { path, error } => Line::err(format!(
            "⏭️  Se omite la carpeta '{}' (no se pudo leer): {}",
            path.display(),
            error
        )),
        ScanWarning::NoDate {
            path,
            source,
            error,
        } => Line::err(format!(
            "⏭️  Se omite '{}' (no se pudo leer su fecha de {}): {}",
            path.display(),
            match source {
                DateSource::Modified => "modificación",
                DateSource::Created => "creación",
            },
            error
        )),
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
            // Relativa a la base: en modo recursivo distingue archivos
            // homónimos de distintas subcarpetas; sin él es solo el nombre.
            let shown = planned
                .source
                .strip_prefix(&plan.base)
                .unwrap_or(&planned.source);
            println!("      {}", shown.display());
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

pub fn event_line(event: &Event) -> Line {
    match event {
        Event::FolderCreated { folder_name } => {
            Line::out(format!("📁 Carpeta creada: {folder_name}"))
        }
        Event::FolderRemoved { folder_name } => {
            Line::out(format!("🗑️  Carpeta vacía eliminada: {folder_name}"))
        }
        Event::Conflict {
            original,
            renamed_to,
            late: false,
        } => Line::out(format!(
            "⚠️  Conflicto: '{}' ya existe → renombrando a '{}'",
            original.to_string_lossy(),
            renamed_to.to_string_lossy()
        )),
        Event::Conflict {
            original,
            renamed_to,
            late: true,
        } => Line::out(format!(
            "⚠️  Conflicto: '{}' apareció en el destino → reintentando como '{}'",
            original.to_string_lossy(),
            renamed_to.to_string_lossy()
        )),
        Event::Moved { from, to } => Line::out(format!("✔ {} → {}", from.display(), to.display())),
        Event::SourceMissing { path } => {
            Line::out(format!("⏭️  Archivo ya no existe: {}", path.display()))
        }
        Event::Failed { source, failure } => Line::err(failure_text(source, failure)),
    }
}

fn failure_text(source: &Path, failure: &Failure) -> String {
    let source = source.display();
    match failure {
        Failure::DestNotADirectory { dest_dir } => format!(
            "✘ '{}' ya existe pero no es una carpeta; se omite '{}'",
            dest_dir.display(),
            source
        ),
        Failure::CreateDir { dest_dir, error } => format!(
            "✘ Error creando carpeta '{}': {}",
            dest_dir.display(),
            error
        ),
        Failure::NoFileName => {
            format!("✘ Se omite '{source}': la ruta no tiene nombre de archivo")
        }
        Failure::ResolveConflict(error) => format!("✘ Error resolviendo conflicto: {error}"),
        Failure::DestinationBusy { attempts } => format!(
            "✘ No se pudo mover '{source}': el destino sigue ocupado tras {attempts} intentos"
        ),
        Failure::CopiedButOriginalRemains { dest, error } => format!(
            "⚠️  '{}' se copió a '{}' pero no se pudo borrar el original (queda duplicado): {}",
            source,
            dest.display(),
            error
        ),
        Failure::Move { rename, copy } => {
            format!("✘ Error moviendo '{source}': {copy} (rename falló con: {rename})")
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

pub fn undo_hint(base: &Path) {
    println!(
        "↩️  Puede deshacerlo con: file-organizer-by-date \"{}\" --undo",
        base.display()
    );
}

pub fn unrecorded(count: usize) {
    eprintln!(
        "⚠️  {count} movimiento(s) con nombres no UTF-8 no se registraron y no podrán deshacerse."
    );
}

pub fn log_error(error: &io::Error) {
    eprintln!("✘ No se pudo abrir el archivo de log: {error}");
}

pub fn journal_error(error: &io::Error) {
    eprintln!("✘ Error con el registro de deshacer ({JOURNAL_FILE_NAME}): {error}");
}

pub fn nothing_to_undo() {
    println!("\n⚠️  No hay nada que deshacer en esa carpeta.");
}

/// Lista lo que se restaurará: `destino actual → ubicación original`.
pub fn undo_preview(journal: &Journal) {
    // Si alguien editó la fecha a mano, se muestra tal cual
    let when = chrono::DateTime::parse_from_rfc3339(&journal.created_at)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|_| journal.created_at.clone());
    println!("\n↩️  Deshacer la ejecución del {when}:");
    println!("{SEPARATOR}");
    for entry in journal.moves.iter().take(MAX_UNDO_PREVIEW) {
        println!("   {} → {}", entry.to.display(), entry.from.display());
    }
    if journal.moves.len() > MAX_UNDO_PREVIEW {
        println!("   ... y {} más", journal.moves.len() - MAX_UNDO_PREVIEW);
    }
    println!("{SEPARATOR}");
    println!(
        "📊 Se restaurarán {} archivo(s) a su ubicación original.\n",
        journal.moves.len()
    );
}
