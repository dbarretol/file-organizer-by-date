use std::path::PathBuf;

use clap::{ArgAction, Parser, ValueEnum};
use file_organizer_by_date::{DateSource, ExtensionFilter, Granularity, Options};

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

    /// Fecha que decide la carpeta: modified (por defecto) o created
    #[arg(
        long,
        value_enum,
        value_name = "FECHA",
        default_value_t = DateArg::Modified,
        hide_default_value = true,
        hide_possible_values = true
    )]
    pub date: DateArg,

    /// Solo mueve archivos con estas extensiones, separadas por comas
    /// (p. ej. jpg,png)
    #[arg(short, long, value_name = "EXTS", value_delimiter = ',')]
    pub extensions: Vec<String>,

    /// Incluye los archivos de las subcarpetas (salta las carpetas Y####…
    /// ya organizadas) y los mueve a las carpetas por fecha de RUTA
    #[arg(short, long)]
    pub recursive: bool,

    /// Deshace la última ejecución sobre RUTA (requiere RUTA)
    #[arg(
        long,
        requires = "path",
        conflicts_with_all = ["group_by", "by_day", "date", "extensions", "recursive"]
    )]
    pub undo: bool,

    /// Añade a ese archivo un registro con marca de tiempo de lo ocurrido
    #[arg(long, value_name = "ARCHIVO")]
    pub log: Option<PathBuf>,

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

/// Reflejo de `DateSource`.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateArg {
    Modified,
    Created,
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
            date_source: match self.date {
                DateArg::Modified => DateSource::Modified,
                DateArg::Created => DateSource::Created,
            },
            extensions: (!self.extensions.is_empty())
                .then(|| ExtensionFilter::new(&self.extensions)),
            recursive: self.recursive,
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
        let options = cli.options();
        assert_eq!(options.granularity, Granularity::Month);
        assert_eq!(options.date_source, DateSource::Modified);
        assert!(options.extensions.is_none());
        assert!(!options.recursive);
    }

    #[test]
    fn scan_options_are_mapped() {
        let cli = parse(&[
            "-r",
            "--date",
            "created",
            "--extensions",
            "jpg,.PNG",
            "-e",
            "gif",
        ])
        .unwrap();
        let options = cli.options();
        assert!(options.recursive);
        assert_eq!(options.date_source, DateSource::Created);
        assert_eq!(
            options.extensions.unwrap().extensions(),
            ["gif", "jpg", "png"]
        );
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
    fn undo_requires_path_and_rejects_scan_options() {
        assert!(parse(&["carpeta", "--undo", "-y"]).unwrap().undo);
        assert_eq!(
            parse(&["--undo"]).unwrap_err().kind(),
            ErrorKind::MissingRequiredArgument
        );
        assert_eq!(
            parse(&["carpeta", "--undo", "-r"]).unwrap_err().kind(),
            ErrorKind::ArgumentConflict
        );
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
