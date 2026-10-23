# 📂 File Organizer by Date

[![CI](https://github.com/dbarretol/file-organizer-by-date/actions/workflows/ci.yml/badge.svg)](https://github.com/dbarretol/file-organizer-by-date/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Rust 1.82+](https://img.shields.io/badge/rust-1.82%2B-orange.svg)

**Tidy up messy folders in one command.** File Organizer by Date moves the
files of a folder (your Downloads, a camera dump, an old desktop…) into
year / month / day subfolders based on each file's date. It always shows you a
preview first, never overwrites anything, and can undo the whole run with a
single command.

```text
Before:                          After:
Downloads/                       Downloads/
├── photo.jpg                    ├── Y2024-M01/
├── report.pdf                   │   ├── photo.jpg
├── notes.txt                    │   └── notes.txt
├── video.mp4                    ├── Y2024-M03/
├── backup.zip                   │   └── report.pdf
└── my-folder/                   ├── Y2025-M11/
    └── stuff.txt                │   └── video.mp4
                                 ├── Y2026-M04/
                                 │   └── backup.zip
                                 └── my-folder/        ← untouched by default
                                     └── stuff.txt
```

---

## ✨ Highlights

- 🔍 **Preview first.** You see exactly which file goes where before anything
  moves, and nothing happens until you confirm.
- 🛡️ **Never overwrites.** Name clashes get a numeric suffix (`photo_1.jpg`),
  even if another program creates a file at the same moment.
- ↩️ **Undo.** `--undo` puts every file back where it was and removes the date
  folders that were left empty.
- 🎛️ **Flexible.**
  - Group by year, month or day.
  - Use the modification or the creation date.
  - Only certain file types (`-e jpg,png`).
  - Include subfolders (`-r`).
- 🤖 **Script-friendly.** Unattended mode (`--yes`), clear exit codes and an
  optional log file.
- 🌍 **Cross-platform.** Windows, Linux and macOS, as a single binary with no
  runtime dependencies.

---

## 🚀 Quick start

```bash
# 1. Install (requires Rust 1.82+; see Installation below)
cargo install --git https://github.com/dbarretol/file-organizer-by-date

# 2. Organize a folder: shows a preview and asks for confirmation
file-organizer-by-date "C:\Users\me\Downloads"

# 3. Not happy with the result? Undo it
file-organizer-by-date "C:\Users\me\Downloads" --undo
```

Run it **without arguments** for an interactive mode that asks for the folder.

---

## 📦 Installation

You need [Rust](https://www.rust-lang.org/tools/install) **1.82 or newer**.

**Option A: install the command** (puts `file-organizer-by-date` on your `PATH`):

```bash
cargo install --git https://github.com/dbarretol/file-organizer-by-date
```

**Option B: build from a clone:**

```bash
git clone https://github.com/dbarretol/file-organizer-by-date.git
cd file-organizer-by-date
cargo build --release
# Binary: target/release/file-organizer-by-date(.exe)
```

---

## 📖 Usage

```text
file-organizer-by-date [OPTIONS] [PATH]
```

| Option               | Description                                                     |
|----------------------|-----------------------------------------------------------------|
| `PATH`               | Folder to organize. Without it, interactive mode starts         |
| `-y`, `--yes`        | Don't ask for confirmation (requires `PATH`)                    |
| `--group-by <LEVEL>` | Folder granularity: `year`, `month` (default) or `day`          |
| `--by-day`           | Shortcut for `--group-by day`                                   |
| `--date <DATE>`      | Date to group by: `modified` (default) or `created`             |
| `-e`, `--extensions` | Only move files with these extensions, comma-separated          |
| `-r`, `--recursive`  | Also collect files from subfolders                              |
| `--undo`             | Undo the last run on `PATH` (requires `PATH`)                   |
| `--log <FILE>`       | Append a timestamped record of every action to `FILE`           |
| `-h`, `--help`       | Print help                                                      |
| `-V`, `--version`    | Print version                                                   |

Quote paths that contain spaces. The grouping and filtering options also apply
in interactive mode.

### Examples

```bash
# One folder per day (Y2024-M01-D15) instead of per month
file-organizer-by-date "C:\Users\me\Downloads" --by-day

# Only photos, including those in subfolders, grouped by creation date
file-organizer-by-date "D:\Camera" -r -e jpg,png,heic --date created

# Unattended (e.g. a scheduled task), keeping a log
file-organizer-by-date ~/Downloads --yes --log ~/organizer.log
```

### What a run looks like

```text
🔍 Preview:
─────────────────────────────────────────────
   📁 Y2024-M07/  (2 file(s))
      image.png
      report.pdf
   📁 Y2025-M12/  (1 file(s))
      notes.txt
─────────────────────────────────────────────
📊 3 file(s) will be moved in total.

Do you want to continue? (y/n): y
📁 Folder created: Y2024-M07
✔ C:\Users\me\Downloads\image.png → C:\Users\me\Downloads\Y2024-M07\image.png
✔ C:\Users\me\Downloads\report.pdf → C:\Users\me\Downloads\Y2024-M07\report.pdf
📁 Folder created: Y2025-M12
✔ C:\Users\me\Downloads\notes.txt → C:\Users\me\Downloads\Y2025-M12\notes.txt

✅ Result:
   Files moved:  3
   Skipped:      0
   Errors:       0
↩️  To undo this run: file-organizer-by-date "C:\Users\me\Downloads" --undo
```

### Exit codes

Useful when calling the tool from scripts:

| Code | Meaning                                                                   |
|------|---------------------------------------------------------------------------|
| `0`  | Success: files moved without errors, or nothing to do                     |
| `1`  | Something went wrong: move errors, cancelled by the user, or invalid path |
| `2`  | Invalid arguments (e.g. two paths, `--yes` without a path)                |

---

## ⚙️ How it works

### Which files are moved

- **By default, only the files directly inside the folder.** Subfolders and
  their contents are left alone.
- **With `--recursive`, files in subfolders are collected too** and moved into
  the date folders **of the folder you passed**, which flattens the
  structure. Emptied subfolders are left in place.
- **Never touched:**
  - symbolic links;
  - folders that already look like date folders (`Y2024`, `Y2024-M01`,
    `Y2024-M01-D15`), even in recursive mode, so running the tool twice
    changes nothing;
  - the tool's own executable.

### Folder names

| `--group-by`       | Pattern                   | Example         |
|--------------------|---------------------------|-----------------|
| `year`             | `Y<year>`                 | `Y2024`         |
| `month` (default)  | `Y<year>-M<month>`        | `Y2024-M01`     |
| `day` / `--by-day` | `Y<year>-M<month>-D<day>` | `Y2024-M01-D15` |

The date is the file's **last modification date**, or its **creation date**
with `--date created`. Creation dates are not available on every platform or
filesystem; files without one are skipped with a warning.

### Undo

- **Every run that moves files saves a record** to
  `.file-organizer-journal.json` inside the organized folder. `--undo` reads
  it, shows what will be restored and asks for confirmation.
- **Only the last run can be undone.** A run that moves nothing keeps the
  previous record.
- **Undo never overwrites either.** If something now occupies a file's
  original location, the restored file gets a numeric suffix.
- **Only date folders that are left empty are removed.** Anything you added to
  them afterwards stays.
- **If some files can't be restored** (e.g. they are open in another program),
  they stay in the record so you can run `--undo` again.

### Safety

- **Moves across drives** fall back to copy + delete. The original
  modification date and permissions are preserved, and the copy is removed if
  it fails halfway.
- **File names that aren't valid UTF-8** (rare, Linux) are organized normally
  but can't be stored in the JSON record. The tool warns that those moves
  can't be undone.

> ⚠️ Undo exists, but moving files is still a big operation. **Try it on a
> copy of a folder first**, and review the preview before confirming.

---

## 🛠️ Development

```bash
cargo test                                  # unit tests
cargo clippy --all-targets -- -D warnings   # lints
cargo fmt --check                           # formatting
```

CI runs these on Windows, Linux and macOS, plus a build with the minimum
supported Rust version (1.82).

### Architecture

The code is split into a **core library**, which never prints or reads input,
and a **CLI** on top of it. The core returns data (`Plan`, `Summary`) and emits
`Event`s; the CLI decides how to show them. The planned desktop GUI will reuse
the same core.

```text
src/
├── lib.rs          # Public API of the core
├── plan.rs         # scan(): folder → Plan of moves + warnings
├── executor.rs     # execute(): Plan → Events + Summary
├── event.rs        # Event / Failure types emitted by the core
├── fs_ops.rs       # Safe moves: never overwrite, race-safe, copy fallback
├── grouping.rs     # Date → destination folder name
├── options.rs      # Options: granularity, date source, filter, recursion
├── filter.rs       # Extension filter
├── journal.rs      # Record of the last run + undo()
├── main.rs         # CLI entry point
└── cli/            # Argument parsing, prompts, output rendering, log file
```

Design documents:
[`docs/REFACTOR_PLAN.md`](docs/REFACTOR_PLAN.md) (architecture and history)
and [`docs/GUI_DESIGN.md`](docs/GUI_DESIGN.md) (future desktop app).

**Dependencies:**
- [chrono](https://crates.io/crates/chrono): dates;
- [clap](https://crates.io/crates/clap): command-line parsing;
- [serde](https://crates.io/crates/serde) and
  [serde_json](https://crates.io/crates/serde_json): the undo record.

---

## 🗺️ Roadmap

- [x] Preview before moving, interactive and script modes
- [x] Group by year / month / day, modification or creation date
- [x] Extension filter and recursive mode
- [x] Undo and log file
- [ ] Desktop GUI, designed in [`docs/GUI_DESIGN.md`](docs/GUI_DESIGN.md)
- [ ] Group photos by EXIF capture date
- [ ] Prebuilt binaries and publishing to [crates.io](https://crates.io)

---

## 🤝 Contributing

Issues and pull requests are welcome. Before opening a PR, please make sure
`cargo test`, `cargo clippy --all-targets -- -D warnings` and
`cargo fmt --check` pass.

## 📄 License

[MIT](LICENSE)
