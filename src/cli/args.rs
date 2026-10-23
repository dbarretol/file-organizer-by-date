use std::ffi::OsString;
use std::path::PathBuf;

pub enum Command {
    Help,
    Interactive,
    /// Modo scriptable: se procesa una sola carpeta y se termina, sin el
    /// bucle interactivo. Con `auto_confirm` (--yes / -y) no se pide
    /// confirmación (necesario en scripts sin stdin); sin el flag se conserva
    /// el prompt como red de seguridad.
    Once {
        path: PathBuf,
        auto_confirm: bool,
    },
}

pub enum UsageError {
    MultiplePaths,
    YesWithoutPath,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, UsageError> {
    let mut auto_confirm = false;
    let mut path: Option<PathBuf> = None;

    for arg in args {
        if arg == "--help" || arg == "-h" {
            return Ok(Command::Help);
        } else if arg == "--yes" || arg == "-y" {
            auto_confirm = true;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(UsageError::MultiplePaths);
        }
    }

    match path {
        Some(path) => Ok(Command::Once { path, auto_confirm }),
        None if auto_confirm => Err(UsageError::YesWithoutPath),
        None => Ok(Command::Interactive),
    }
}
