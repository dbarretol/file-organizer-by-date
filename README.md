# 📂 File Organizer by Date

A fast, cross-platform command-line tool written in **Rust** that automatically
organizes **loose files** in a directory into folders based on their
**last modification date**.

```text
Before:                          After:
├── photo.jpg                    ├── Y2024-M01/
├── report.pdf                   │   ├── photo.jpg
├── notes.txt                    │   └── notes.txt
├── video.mp4                    ├── Y2024-M03/
├── backup.zip                   │   └── report.pdf
└── my-folder/                   ├── Y2025-M11/
    └── stuff.txt                │   └── video.mp4
                                 ├── Y2026-M04/
                                 │   └── backup.zip
                                 └── my-folder/        ← untouched
                                     └── stuff.txt     ← untouched
```

---

## ✨ Features

- 🦀 **Written in Rust** — fast, safe, and reliable.
- 🌍 **Cross-platform** — works on Windows, Linux, and macOS.
- 💬 **Interactive mode** — prompts for the directory path and confirms before
  moving files.
- 🤖 **Scriptable CLI mode** — pass the path as an argument (with optional
  `--yes` to skip confirmation) and check the exit code (0/1/2) from scripts.
- 🔍 **Dry-run preview** — shows exactly what will happen before any file is
  moved, grouped by destination folder.
- 📄 **Loose files by default** — only moves files directly inside the target
  folder; subdirectories are untouched unless you pass `--recursive`.
- 🗂️ **Recursive mode** — `--recursive` also collects files from subfolders
  (skipping already-organized `YXXXX…` folders) into the date folders of the
  target directory.
- 🎯 **Extension filter** — `--extensions jpg,png` moves only matching files
  (case-insensitive).
- 📅 **Date source** — group by modification date (default) or by creation
  date with `--date created`.
- 📁 **Auto-creates folders** — generates `YXXXX-MXX` directories automatically
  (or `YXXXX` / `YXXXX-MXX-DXX` with `--group-by year|day`).
- 🛡️ **Name conflict resolution** — appends numeric suffixes (`_1`, `_2`...) to
  avoid overwriting files.
- 🚫 **Self-protection** — the executable never moves itself.
- 💾 **Cross-filesystem support** — falls back to copy + delete when `rename`
  fails across different drives, preserving the original modification date and
  permissions.
- 🔗 **Symlinks left alone** — symbolic links are skipped (moving them could
  break relative targets and their date would be the target's, not the link's).
- ↩️ **Undo** — every run records its moves in `.file-organizer-journal.json`
  inside the target folder; `--undo` puts the files back (recreating
  subfolders) and removes the date folders it emptied.
- 📝 **Log file** — `--log FILE` appends a timestamped record of every action.
- 📊 **Summary report** — displays a final count of moved, skipped, and failed
  files.
- 🔄 **Loop mode** — organize multiple folders in one session without restarting.

---

## 📋 Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.82 or newer (required by
  the Windows dependencies of `chrono`; CI verifies this minimum)

---

## 🚀 Installation

```bash
# Clone the repository
git clone https://github.com/your-username/file-organizer-by-date.git
cd file-organizer-by-date

# Build in release mode
cargo build --release
```

The compiled binary will be located at:

```
target/release/file-organizer-by-date      # Linux / macOS
target/release/file-organizer-by-date.exe  # Windows
```

---

## 📖 Usage

```text
file-organizer-by-date [OPTIONS] [PATH]
```

| Option               | Description                                            |
|----------------------|--------------------------------------------------------|
| `PATH`               | Organize that folder and exit (script mode)            |
| `-y`, `--yes`        | Don't ask for confirmation (requires `PATH`)           |
| `--group-by <LEVEL>` | Folder granularity: `year`, `month` (default) or `day` |
| `--by-day`           | Shortcut for `--group-by day`                          |
| `--date <DATE>`      | Date used for grouping: `modified` (default) or `created` |
| `-e`, `--extensions` | Only move files with these extensions (`jpg,png`)      |
| `-r`, `--recursive`  | Also collect files from subfolders                     |
| `--undo`             | Undo the last run on `PATH` (requires `PATH`)          |
| `--log <FILE>`       | Append a timestamped log of every action to `FILE`     |
| `-h`, `--help`       | Print help                                             |
| `-V`, `--version`    | Print version                                          |

Run without arguments for **interactive mode**, or pass a path to organize a
single folder and exit (**script mode**):

```bash
# Interactive mode
./target/release/file-organizer-by-date

# Script mode: shows the preview, asks for confirmation, then exits
./target/release/file-organizer-by-date "C:\Users\me\Downloads"

# Fully unattended: no confirmation prompt (use with care!)
./target/release/file-organizer-by-date "C:\Users\me\Downloads" --yes

# One folder per day (Y2024-M01-D15) instead of per month
./target/release/file-organizer-by-date "C:\Users\me\Downloads" --by-day

# Only photos, including those in subfolders, grouped by creation date
./target/release/file-organizer-by-date "D:\Camera" -r -e jpg,png,heic --date created

# Changed your mind? Put everything back where it was
./target/release/file-organizer-by-date "D:\Camera" --undo
```

Quote the path if it contains spaces. All grouping and filtering options also
apply to interactive mode.

### Exit codes

Script mode returns a meaningful exit code so automation can detect failures:

| Code | Meaning                                                            |
|------|--------------------------------------------------------------------|
| `0`  | Success (files moved without errors, or nothing to organize)       |
| `1`  | Something went wrong: move errors, cancelled/EOF, or invalid path  |
| `2`  | Incorrect arguments (e.g. multiple paths, `--yes` without a path, unknown `--group-by` value) |

### Interactive flow

```text
╔═══════════════════════════════════════════════╗
║       📂 File Organizer by Date v0.5.0        ║
║  Organiza archivos por fecha de modificación  ║
╚═══════════════════════════════════════════════╝

📁 Ingrese la ruta de la carpeta a organizar (o 'q' para salir): C:\Users\me\Downloads

🔍 Vista previa:
─────────────────────────────────────────────
   📁 Y2024-M07/  (2 archivo(s))
      image.png
      report.pdf
   📁 Y2025-M12/  (1 archivo(s))
      notes.txt
─────────────────────────────────────────────
📊 Se moverán 3 archivo(s) en total.

¿Está seguro de que desea continuar? (s/n): s
📁 Carpeta creada: Y2024-M07
✔ C:\Users\me\Downloads\image.png → C:\Users\me\Downloads\Y2024-M07\image.png
✔ C:\Users\me\Downloads\report.pdf → C:\Users\me\Downloads\Y2024-M07\report.pdf
📁 Carpeta creada: Y2025-M12
✔ C:\Users\me\Downloads\notes.txt → C:\Users\me\Downloads\Y2025-M12\notes.txt

✅ Resultado:
   Archivos movidos:  3
   Omitidos:          0
   Errores:           0

─────────────────────────────────────────────
¿Desea organizar otra carpeta? (s/n, o presione 'q' para salir): q
👋 ¡Hasta luego!
```

---

## 📁 Folder Naming Convention

By default files are organized into folders using the pattern
`Y<year>-M<month>`. `--group-by` changes the granularity:

| `--group-by`       | Pattern                   | Example         |
|--------------------|---------------------------|-----------------|
| `year`             | `Y<year>`                 | `Y2024`         |
| `month` (default)  | `Y<year>-M<month>`        | `Y2024-M01`     |
| `day` / `--by-day` | `Y<year>-M<month>-D<day>` | `Y2024-M01-D15` |

---

## 🏗️ Project Structure

```text
file-organizer-by-date/
├── Cargo.toml          # Dependencies and project metadata
├── README.md           # This file
├── docs/
│   └── REFACTOR_PLAN.md
└── src/
    ├── lib.rs          # Core library API (no printing, no stdin)
    ├── plan.rs         # scan(): directory → Plan of moves + warnings
    ├── executor.rs     # execute(): Plan → Events + Summary
    ├── event.rs        # Event / Failure types emitted by the core
    ├── fs_ops.rs       # Safe moves: never overwrite, TOCTOU, copy fallback
    ├── grouping.rs     # Date → destination folder name
    ├── options.rs      # Options: granularity, date source, filter, recursion
    ├── filter.rs       # Extension filter
    ├── journal.rs      # Journal of the last run + undo()
    ├── main.rs         # CLI entry point
    └── cli/            # Argument parsing, prompts and text rendering
```

The core (`lib.rs` and its modules) returns data and emits events instead of
printing, so other frontends (e.g. a GUI) can reuse it as-is.

---

## ⚙️ Dependencies

| Crate                                     | Purpose                          |
|-------------------------------------------|----------------------------------|
| [chrono](https://crates.io/crates/chrono) | Date/time formatting and parsing |
| [clap](https://crates.io/crates/clap)     | Command-line argument parsing    |
| [serde](https://crates.io/crates/serde) + [serde_json](https://crates.io/crates/serde_json) | Undo journal (JSON) |

---

## ⚠️ Important Notes

> **Always test on a copy of your files first.** Moving files is a destructive
> operation that may be difficult to undo.

- By default the tool **only moves loose files** directly inside the target
  directory. With `--recursive` it also collects files from subfolders and
  moves them **to the date folders of the target directory** (the subfolder
  structure is flattened; emptied subfolders are left in place).
- The tool uses the file's **last modification date** unless you pass
  `--date created`. Creation dates are not available on every platform or
  filesystem; files without one are skipped with a warning.
- Running the tool multiple times is safe — files already inside `YXXXX`,
  `YXXXX-MXX` or `YXXXX-MXX-DXX` folders are never touched, even in recursive
  mode.
- If a file with the same name already exists in the destination folder, a
  numeric suffix is added (up to `_10000`).
- **Undo covers the last run that moved files** in a folder. A run that moves
  nothing keeps the previous journal. Undo never overwrites: if a file now
  occupies an original location, the restored file gets a numeric suffix.
  Date folders are removed only if they are empty, so anything you added to
  them afterwards is left alone. Moves that fail stay in the journal so you
  can retry `--undo`.
- File names that aren't valid UTF-8 are organized normally but can't be
  recorded in the JSON journal; the tool warns that those moves can't be
  undone.
- The dry-run preview lets you review changes before committing.

---

## 🤝 Contributing

Contributions are welcome! Feel free to:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).

---

## 🗺️ Roadmap

- [x] ~~Dry-run preview before moving files~~
- [x] ~~Interactive mode with confirmation prompt~~
- [x] ~~Loop mode to organize multiple folders~~
- [x] ~~Add `--by-day` flag to organize by day (`Y2024-M01-D15`)~~
- [x] ~~Add `--undo` flag to reverse the last organization~~
- [x] ~~Add `--extensions` flag to filter by file type~~
- [x] ~~Add recursive mode with `--recursive` flag~~
- [x] ~~Add logging to a file~~
- [ ] Publish to [crates.io](https://crates.io)

---

<p align="center">
  Made with 🦀 and ❤️
</p>