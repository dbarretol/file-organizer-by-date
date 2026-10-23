use std::io::{self, Write};
use std::path::PathBuf;

/// Pide una carpeta hasta recibir una válida. `None` si el usuario sale
/// ('q') o stdin se agota.
pub fn ask_for_directory() -> Option<PathBuf> {
    loop {
        let input = read_line("📁 Ingrese la ruta de la carpeta a organizar (o 'q' para salir): ")?;
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

/// Pregunta sí/no hasta obtener una respuesta reconocible. EOF cuenta como "no".
pub fn confirm(prompt: &str) -> bool {
    loop {
        let Some(input) = read_line(prompt) else {
            return false;
        };

        match input.trim().to_lowercase().as_str() {
            "s" | "si" | "sí" | "y" | "yes" => return true,
            "n" | "no" | "q" | "quit" | "exit" => return false,
            _ => println!("   Por favor, responda 's' (sí) o 'n' (no)."),
        }
    }
}

/// Muestra el prompt y lee una línea. `None` en EOF (Ctrl+Z / pipe agotado),
/// para salir en lugar de reintentar para siempre.
fn read_line(prompt: &str) -> Option<String> {
    print!("{prompt}");
    // Un stdout cerrado (pipe roto) no debe hacer entrar en pánico
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
        println!();
        return None;
    }
    Some(input)
}
