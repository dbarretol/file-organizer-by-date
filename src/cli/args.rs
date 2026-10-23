use std::path::PathBuf;

use clap::{ArgAction, Parser, ValueEnum};
use file_organizer_by_date::{Granularity, Options};

const EXIT_CODES_HELP: &str = "\
Códigos de salida:
  0  Éxito (o nada que organizar)
  1  Hubo errores al mover, se canceló, o la ruta no es válida
  2  Argumentos incorrectos";

/// Organiza los archivos sueltos de una carpeta en subcarpetas por fecha
/// de modificación.
#[derive(Parser, Debug)]
#[command(
    name = "file-organizer-by-date",
    bin_name = "file-organizer-by-date",
    version,
    override_usage = "file-organizer-by-date [OPCIONES] [RUTA]",
    help_template = "{about}\n\nUso: {usage}\n\n{all-args}{after-help}",
    after_help = EXIT_CODES_HELP,
    disable_help_flag = true,
    disable_version_flag = true,
    next_help_heading = "Opciones"
)]
pub struct Cli {
    /// Organiza esa carpeta y termina (modo script). Sin ella se inicia el
    /// modo interactivo
    #[arg(value_name = "RUTA", help_heading = "Argumentos")]
    pub path: Option<PathBuf>,

    /// No pide confirmación antes de mover (requiere RUTA)
    #[arg(short, long, requires = "path")]
    pub yes: bool,

    /// Nivel de detalle de las carpetas: year (Y2024), month (Y2024-M01,
    /// por defecto) o day (Y2024-M01-D15)
    #[arg(
        long,
        value_enum,
        value_name = "NIVEL",
        default_value_t = GroupBy::Month,
        hide_default_value = true,
        hide_possible_values = true
    )]
    pub group_by: GroupBy,

    /// Atajo de --group-by day (carpetas Y2024-M01-D15)
    #[arg(long, conflicts_with = "group_by")]
    pub by_day: bool,

    /// Muestra esta ayuda
    #[arg(short, long, action = ArgAction::Help)]
    help: Option<bool>,

    /// Muestra la versión
    #[arg(short = 'V', long, action = ArgAction::Version)]
    version: Option<bool>,
}

/// Reflejo de `Granularity` para la línea de comandos: el núcleo no depende
/// de clap.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupBy {
    Year,
    Month,
    Day,
}

impl Cli {
    pub fn options(&self) -> Options {
        let group_by = if self.by_day {
            GroupBy::Day
        } else {
            self.group_by
        };
        Options {
            granularity: match group_by {
                GroupBy::Year => Granularity::Year,
                GroupBy::Month => Granularity::Month,
                GroupBy::Day => Granularity::Day,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("fo").chain(args.iter().copied()))
    }

    #[test]
    fn definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn defaults_to_interactive_month_grouping() {
        let cli = parse(&[]).unwrap();
        assert!(cli.path.is_none());
        assert!(!cli.yes);
        assert_eq!(cli.options().granularity, Granularity::Month);
    }

    #[test]
    fn yes_requires_a_path() {
        let err = parse(&["--yes"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
        assert!(parse(&["C:\\x", "-y"]).unwrap().yes);
    }

    #[test]
    fn rejects_a_second_path() {
        let err = parse(&["a", "b"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn by_day_and_group_by_select_granularity() {
        let by_day = parse(&["--by-day"]).unwrap();
        assert_eq!(by_day.options().granularity, Granularity::Day);

        let year = parse(&["--group-by", "year"]).unwrap();
        assert_eq!(year.options().granularity, Granularity::Year);

        let err = parse(&["--by-day", "--group-by", "year"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }
}
