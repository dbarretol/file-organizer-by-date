//! Frontend de consola sobre el núcleo `file_organizer_by_date`.

mod args;
mod prompt;
mod render;

use std::path::Path;
use std::process::ExitCode;

use clap::error::{ContextKind, ErrorKind};
use clap::Parser;
use file_organizer_by_date::{execute, scan, Options};

use args::Cli;

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

    match &cli.path {
        Some(path) => {
            if !path.is_dir() {
                render::invalid_dir(path);
                return ExitCode::from(EXIT_FAILURE);
            }
            // 0 solo si no hubo errores ni cancelación
            if organize_directory(path, &options, cli.yes) {
                ExitCode::from(EXIT_OK)
            } else {
                ExitCode::from(EXIT_FAILURE)
            }
        }
        None => {
            interactive_loop(&options);
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

fn interactive_loop(options: &Options) {
    while let Some(target_dir) = prompt::ask_for_directory() {
        organize_directory(&target_dir, options, false);

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
fn organize_directory(target_dir: &Path, options: &Options, auto_confirm: bool) -> bool {
    let plan = match scan(target_dir, options) {
        Ok(plan) => plan,
        Err(error) => {
            render::scan_error(&error);
            return false;
        }
    };
    render::warnings(&plan.warnings);

    if plan.is_empty() {
        render::nothing_to_do();
        return true;
    }

    render::preview(&plan);

    if !auto_confirm && !prompt::confirm("¿Está seguro de que desea continuar? (s/n): ") {
        render::cancelled();
        return false;
    }

    let summary = execute(&plan, &mut render::event);
    render::summary(&summary);
    summary.is_success()
}
