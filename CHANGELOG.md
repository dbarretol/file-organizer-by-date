# Changelog

All notable changes to this project are documented here. The format is based
on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/).

## [0.6.0] - 2026-10-22

### Added

- `--group-by year|month|day` and the `--by-day` shortcut to choose the folder
  granularity (`Y2024`, `Y2024-M01`, `Y2024-M01-D15`).
- `--date modified|created` to group by modification or creation date.
- `-e`, `--extensions jpg,png` to move only files with certain extensions
  (case-insensitive).
- `-r`, `--recursive` to also collect files from subfolders, skipping
  already-organized date folders.
- `--undo` to put every file of the last run back where it was. Each run
  records its moves in `.file-organizer-journal.json` inside the folder.
- `--log FILE` to append a timestamped record of every action.
- `-V`, `--version`.
- CI on Windows, Linux and macOS, plus a minimum-supported-Rust build.

### Changed

- All on-screen messages, prompts and `--help` are now in English. Prompts
  accept `y`/`yes` and `n`/`no`.
- Command-line parsing now uses `clap`.
- The code is split into a core library, which never prints or reads input,
  and a CLI on top of it, so a future GUI can reuse the core.
- The minimum supported Rust version is now 1.82.

### Fixed

- File names that aren't valid UTF-8 are no longer corrupted when renamed to
  resolve a name conflict.
- The tool no longer panics when its output is piped to a closed stream.

## [0.5.0] - 2026-07-18

- Interactive and script modes, preview before moving, conflict-safe moves
  that never overwrite, and a copy fallback across drives that keeps the
  modification date.

[0.6.0]: https://github.com/dbarretol/file-organizer-by-date/compare/ff55cb1...v0.6.0
[0.5.0]: https://github.com/dbarretol/file-organizer-by-date/commit/ff55cb1
