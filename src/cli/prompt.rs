use std::io::{self, Write};
use std::path::PathBuf;

/// Asks for a folder until a valid one is given. `None` if the user quits
/// ('q') or stdin runs out.
pub fn ask_for_directory() -> Option<PathBuf> {
    loop {
        let input = read_line("📁 Enter the path of the folder to organize (or 'q' to quit): ")?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("q") {
            return None;
        }

        if input.is_empty() {
            println!("⚠️  The path can't be empty. Please try again.\n");
            continue;
        }

        let cleaned = input.trim_matches('"').trim_matches('\'');
        let path = PathBuf::from(cleaned);

        if !path.exists() {
            println!(
                "⚠️  The path '{}' doesn't exist. Please try again.\n",
                path.display()
            );
            continue;
        }

        if !path.is_dir() {
            println!(
                "⚠️  '{}' is not a folder. Please try again.\n",
                path.display()
            );
            continue;
        }

        return Some(path);
    }
}

/// Asks a yes/no question until it gets a recognizable answer. EOF counts
/// as "no".
pub fn confirm(prompt: &str) -> bool {
    loop {
        let Some(input) = read_line(prompt) else {
            return false;
        };

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => return true,
            "n" | "no" | "q" | "quit" | "exit" => return false,
            _ => println!("   Please answer 'y' (yes) or 'n' (no)."),
        }
    }
}

/// Shows the prompt and reads a line. `None` on EOF (Ctrl+Z / exhausted
/// pipe), so callers exit instead of retrying forever.
fn read_line(prompt: &str) -> Option<String> {
    print!("{prompt}");
    // A closed stdout (broken pipe) must not cause a panic
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
        println!();
        return None;
    }
    Some(input)
}
