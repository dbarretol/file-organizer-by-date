use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use file_organizer_by_date::{DateSource, ExtensionFilter, Granularity, Options};

const EXIT_CODES_HELP: &str = "\
Exit codes:
  0  Success (or nothing to organize)
  1  Errors while moving, cancelled, or invalid path
  2  Invalid arguments";

/// Organizes the loose files of a folder into subfolders by date.
#[derive(Parser, Debug)]
#[command(
    name = "file-organizer-by-date",
    bin_name = "file-organizer-by-date",
    version,
    after_help = EXIT_CODES_HELP
)]
pub struct Cli {
    /// Folder to organize, then exit (script mode). Without it, interactive
    /// mode starts
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Don't ask for confirmation before moving (requires PATH)
    #[arg(short, long, requires = "path")]
    pub yes: bool,

    /// Folder granularity: Y2024, Y2024-M01 or Y2024-M01-D15
    #[arg(long, value_enum, value_name = "LEVEL", default_value_t = GroupBy::Month)]
    pub group_by: GroupBy,

    /// Shortcut for --group-by day
    #[arg(long, conflicts_with = "group_by")]
    pub by_day: bool,

    /// Which file date decides the folder
    #[arg(long, value_enum, value_name = "DATE", default_value_t = DateArg::Modified)]
    pub date: DateArg,

    /// Only move files with these extensions, comma-separated (e.g. jpg,png)
    #[arg(short, long, value_name = "EXTS", value_delimiter = ',')]
    pub extensions: Vec<String>,

    /// Also collect files from subfolders (skipping already-organized Y####…
    /// folders) and move them into the date folders of PATH
    #[arg(short, long)]
    pub recursive: bool,

    /// Undo the last run on PATH (requires PATH)
    #[arg(
        long,
        requires = "path",
        conflicts_with_all = ["group_by", "by_day", "date", "extensions", "recursive"]
    )]
    pub undo: bool,

    /// Append a timestamped record of every action to FILE
    #[arg(long, value_name = "FILE")]
    pub log: Option<PathBuf>,
}

/// Mirror of `Granularity` for the command line: the core doesn't depend on
/// clap.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupBy {
    Year,
    Month,
    Day,
}

/// Mirror of `DateSource`.
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
        assert!(parse(&["folder", "-y"]).unwrap().yes);
    }

    #[test]
    fn rejects_a_second_path() {
        let err = parse(&["a", "b"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn undo_requires_path_and_rejects_scan_options() {
        assert!(parse(&["folder", "--undo", "-y"]).unwrap().undo);
        assert_eq!(
            parse(&["--undo"]).unwrap_err().kind(),
            ErrorKind::MissingRequiredArgument
        );
        assert_eq!(
            parse(&["folder", "--undo", "-r"]).unwrap_err().kind(),
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
