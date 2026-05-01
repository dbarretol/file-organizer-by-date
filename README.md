# 📂 File Organizer by Date

A fast, cross-platform command-line tool written in **Rust** that automatically
organizes files into folders based on their **last modification date**.

```text
Before:                          After:
├── photo.jpg                    ├── Y2024-M01/
├── report.pdf                   │   ├── photo.jpg
├── notes.txt                    │   └── notes.txt
├── video.mp4                    ├── Y2024-M03/
└── backup.zip                   │   └── report.pdf
                                 ├── Y2025-M11/
                                 │   └── video.mp4
                                 └── Y2026-M04/
                                     └── backup.zip
```

---

## ✨ Features

- 🦀 **Written in Rust** — fast, safe, and reliable.
- 🌍 **Cross-platform** — works on Windows, Linux, and macOS.
- 💬 **Interactive mode** — prompts for the directory path and confirms before
  moving files.
- 🔍 **Dry-run preview** — shows exactly what will happen before any file is
  moved.
- 🔁 **Recursive scanning** — processes files in all subdirectories.
- 📁 **Auto-creates folders** — generates `YXXXX-MXX` directories automatically.
- 🛡️ **Name conflict resolution** — appends numeric suffixes (`_1`, `_2`...) to
  avoid overwriting files.
- ♻️ **Safe re-execution** — detects and skips already-organized folders.
- 🚫 **Self-protection** — the executable never moves itself.
- 💾 **Cross-filesystem support** — falls back to copy + delete when `rename`
  fails across different drives.
- 📊 **Summary report** — displays a final count of moved, skipped, and failed
  files.
- 🔄 **Loop mode** — organize multiple folders in one session.

---

## 📋 Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (1.56+ recommended)

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

```bash
cargo run
# or
./target/release/file-organizer-by-date
```

### Interactive flow

```text
╔══════════════════════════════════════════════╗
║       📂 File Organizer by Date v0.2.0      ║
║  Organiza archivos por fecha de modificación ║
╚══════════════════════════════════════════════╝

📁 Ingrese la ruta de la carpeta a organizar (o 'q' para salir): C:\Users\me\Downloads

🔍 Vista previa (dry-run):
─────────────────────────────────────────────
   report.pdf → Y2024-M07/
   image.png → Y2024-M07/
   notes.txt → Y2025-M12/
─────────────────────────────────────────────
📊 Se moverán 3 archivo(s) en total.

¿Está seguro de que desea continuar? (s/n): s

📁 Carpeta creada: Y2024-M07
✔ C:\Users\me\Downloads\report.pdf → C:\Users\me\Downloads\Y2024-M07\report.pdf
✔ C:\Users\me\Downloads\image.png → C:\Users\me\Downloads\Y2024-M07\image.png
📁 Carpeta creada: Y2025-M12
✔ C:\Users\me\Downloads\notes.txt → C:\Users\me\Downloads\Y2025-M12\notes.txt

✅ Resultado:
   Archivos movidos:  3
   Archivos omitidos: 0
   Errores:           0

─────────────────────────────────────────────
¿Desea organizar otra carpeta? (s/n, o presione 'q' para salir): q
👋 ¡Hasta luego!
```

---

## 📁 Folder Naming Convention

Files are organized into folders using the pattern:

```
Y<year>-M<month>
```

| Example Folder | Meaning        |
|----------------|----------------|
| `Y2024-M01`    | January 2024   |
| `Y2025-M12`    | December 2025  |
| `Y2026-M04`    | April 2026     |

---

## 🏗️ Project Structure

```text
file-organizer-by-date/
├── Cargo.toml       # Dependencies and project metadata
├── README.md        # This file
└── src/
    └── main.rs      # Application source code
```

---

## ⚙️ Dependencies

| Crate                                     | Purpose                          |
|-------------------------------------------|----------------------------------|
| [chrono](https://crates.io/crates/chrono) | Date/time formatting and parsing |

---

## ⚠️ Important Notes

> **Always test on a copy of your files first.** Moving files is a destructive
> operation that may be difficult to undo.

- The tool uses the file's **last modification date**, not the creation date.
- Running the tool multiple times is safe — it will skip folders it already
  created.
- If a file with the same name already exists in the destination folder, a
  numeric suffix is added (up to `_10000`).
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

- [x] ~~Add `--dry-run` preview before moving files~~
- [x] ~~Interactive mode with confirmation prompt~~
- [x] ~~Loop mode to organize multiple folders~~
- [ ] Add `--by-day` flag to organize by day (`Y2024-M01-D15`)
- [ ] Add `--undo` flag to reverse the last organization
- [ ] Add `--extensions` flag to filter by file type
- [ ] Add logging to a file
- [ ] Publish to [crates.io](https://crates.io)

---

<p align="center">
  Made with 🦀 and ❤️
</p>