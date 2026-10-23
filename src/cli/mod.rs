//! Frontend de consola sobre el núcleo `file_organizer_by_date`.

mod args;
mod log;
mod prompt;
mod render;

use std::path::Path;
use std::process::ExitCode;

use clap::error::{ContextKind, ErrorKind};
use clap::Parser;
use file_organizer_by_date::{
    execute, scan, undo, Event, Journal, JournalRecorder, Options, Summary,
};

use args::Cli;
use log::EventLog;

// Códigos de salida documentados en `--help` y en el README.
const EXIT_OK: u8 = 0;
const EXIT_FAILURE: u8 = 1;
const EXIT_USAGE: u8 = 2;

pub fn run() -> ExitCode {
    render::banner();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return report_parse_error(&error),
    };
    let options = cli.options();
    let mut log = match EventLog::open(cli.log.as_deref()) {
        Ok(log) => log,
        Err(error) => {
            render::log_error(&error);
            return ExitCode::from(EXIT_FAILURE);
        }
    };

    match &cli.path {
        Some(path) => {
            if !path.is_dir() {
                render::invalid_dir(path);
                return ExitCode::from(EXIT_FAILURE);
            }
            // 0 solo si no hubo errores ni cancelación
            let ok = if cli.undo {
                undo_directory(path, cli.yes, &mut log)
            } else {
                organize_directory(path, &options, cli.yes, &mut log)
            };
            ExitCode::from(if ok { EXIT_OK } else { EXIT_FAILURE })
        }
        None => {
            interactive_loop(&options, &mut log);
            ExitCode::from(EXIT_OK)
        }
    }
}

/// clap entrega --help y --version como "errores" que van a stdout; el
/// resto son errores de uso.
fn report_parse_error(error: &clap::Error) -> ExitCode {
    let _ = error.print();
    if !error.use_stderr() {
        return ExitCode::from(EXIT_OK);
    }
    // Una segunda ruta suele ser una ruta con espacios sin comillas
    let extra_positional = error.kind() == ErrorKind::UnknownArgument
        && error
            .get(ContextKind::InvalidArg)
            .is_some_and(|arg| !arg.to_string().starts_with('-'));
    if extra_positional {
        render::quote_hint();
    }
    ExitCode::from(EXIT_USAGE)
}

fn interactive_loop(options: &Options, log: &mut EventLog) {
    while let Some(target_dir) = prompt::ask_for_directory() {
        organize_directory(&target_dir, options, false, log);

        render::separator();
        if !prompt::confirm("¿Desea organizar otra carpeta? (s/n, o presione 'q' para salir): ") {
            break;
        }
        println!();
    }
    render::goodbye();
}

/// Flujo completo sobre una carpeta: escanear, previsualizar, confirmar, mover.
/// Con `auto_confirm` no se pregunta antes de mover (modo script).
/// Devuelve true si terminó sin errores (incluida una carpeta sin nada que
/// organizar); false si el escaneo falló, hubo errores o se canceló.
fn organize_directory(
    target_dir: &Path,
    options: &Options,
    auto_confirm: bool,
    log: &mut EventLog,
) -> bool {
    log.line(&format!("=== Organizar '{}' ===", target_dir.display()));
    let plan = match scan(target_dir, options) {
        Ok(plan) => plan,
        Err(error) => {
            render::scan_error(&error);
            log.line(&format!("Error al escanear: {error}"));
            return false;
        }
    };
    for warning in &plan.warnings {
        let line = render::warning_line(warning);
        line.print();
        log.line(&line.text);
    }

    if plan.is_empty() {
        render::nothing_to_do();
        log.line("Nada que organizar");
        return true;
    }

    render::preview(&plan);

    if !auto_confirm && !prompt::confirm("¿Está seguro de que desea continuar? (s/n): ") {
        render::cancelled();
        log.line("Cancelado por el usuario");
        return false;
    }

    let mut recorder = JournalRecorder::new(target_dir);
    let summary = execute(&plan, &mut |event| {
        recorder.record(&event);
        show_event(&event, log);
    });
    finish(&summary, log);

    if recorder.unrecorded() > 0 {
        render::unrecorded(recorder.unrecorded());
    }
    match recorder.save() {
        Ok(true) => render::undo_hint(target_dir),
        Ok(false) => {}
        Err(error) => render::journal_error(&error),
    }
    summary.is_success()
}

/// Deshace la última ejecución registrada en `target_dir`.
/// Devuelve true si no hubo errores (o no había nada que deshacer).
fn undo_directory(target_dir: &Path, auto_confirm: bool, log: &mut EventLog) -> bool {
    log.line(&format!("=== Deshacer '{}' ===", target_dir.display()));
    let journal = match Journal::load(target_dir) {
        Ok(Some(journal)) => journal,
        Ok(None) => {
            render::nothing_to_undo();
            log.line("Nada que deshacer");
            return true;
        }
        Err(error) => {
            render::journal_error(&error);
            log.line(&format!("Error leyendo el journal: {error}"));
            return false;
        }
    };

    render::undo_preview(&journal);

    if !auto_confirm && !prompt::confirm("¿Desea deshacer estos movimientos? (s/n): ") {
        render::cancelled();
        log.line("Cancelado por el usuario");
        return false;
    }

    let result = undo(target_dir, &journal, &mut |event| show_event(&event, log));
    finish(&result.summary, log);

    // Lo pendiente queda guardado para reintentar; si no queda nada, el
    // journal se borra
    if let Err(error) = result.pending.persist(target_dir) {
        render::journal_error(&error);
        return false;
    }
    result.summary.is_success()
}

fn show_event(event: &Event, log: &mut EventLog) {
    let line = render::event_line(event);
    line.print();
    log.line(&line.text);
}

fn finish(summary: &Summary, log: &mut EventLog) {
    render::summary(summary);
    log.line(&format!(
        "Resultado: {} movido(s), {} omitido(s), {} error(es)",
        summary.moved, summary.skipped, summary.errors
    ));
}
