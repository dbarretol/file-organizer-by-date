//! Frontend de consola sobre el núcleo `file_organizer_by_date`.

mod args;
mod prompt;
mod render;

use std::env;
use std::path::Path;
use std::process::ExitCode;

use file_organizer_by_date::{execute, scan};

use args::Command;

// Códigos de salida documentados en `--help` y en el README.
const EXIT_OK: u8 = 0;
const EXIT_FAILURE: u8 = 1;
const EXIT_USAGE: u8 = 2;

pub fn run() -> ExitCode {
    render::banner();

    let command = match args::parse(env::args_os().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            render::usage_error(&error);
            return ExitCode::from(EXIT_USAGE);
        }
    };

    match command {
        Command::Help => {
            render::usage();
            ExitCode::from(EXIT_OK)
        }
        Command::Once { path, auto_confirm } => {
            if !path.is_dir() {
                render::invalid_dir(&path);
                return ExitCode::from(EXIT_FAILURE);
            }
            // 0 solo si no hubo errores ni cancelación
            if organize_directory(&path, auto_confirm) {
                ExitCode::from(EXIT_OK)
            } else {
                ExitCode::from(EXIT_FAILURE)
            }
        }
        Command::Interactive => {
            interactive_loop();
            ExitCode::from(EXIT_OK)
        }
    }
}

fn interactive_loop() {
    while let Some(target_dir) = prompt::ask_for_directory() {
        organize_directory(&target_dir, false);

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
fn organize_directory(target_dir: &Path, auto_confirm: bool) -> bool {
    let plan = match scan(target_dir) {
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
