# Refactor plan: reusable core, extensibility and a future GUI

> Working branch: `refactor/core-lib`
> Starting point: v0.5.0 (`ff55cb1`). A single 687-line `src/main.rs`,
> clean `clippy` and 6 passing tests.

## Goals

1. **Maintainability and clarity:** short functions, named types, separate
   responsibilities.
2. **Low-friction features:** each roadmap feature touches a single extension
   point, not three functions at once.
3. **Ready for a GUI:** the core never prints or reads stdin. It returns data
   (`Plan`, `Summary`) and emits events (`Event`) that any frontend can
   consume.

**Cross-cutting rule:** in phases 1 to 3 the CLI's visible behavior doesn't
change (same messages, same exit codes). The existing tests must keep passing
in every phase.

---

## Diagnosis (audit summary)

| # | Problem | Location (v0.5.0) | Impact |
|---|---------|-------------------|--------|
| D1 | The core prints to the console (`println!`/`eprintln!`) | `collect_files`, `execute_moves`, `organize_directory` | A GUI can't reuse the logic |
| D2 | Everything in one file, no `lib` crate | `src/main.rs` | No integration tests, no reuse |
| D3 | `execute_moves` is ~130 lines with nested loops and `continue 'files` | `src/main.rs:184-312` | Hard to read and change |
| D4 | `(u32, u32, u32)` return value | `execute_moves` | You have to remember the order |
| D5 | `process::exit` scattered across `main` | `main` | No single source for exit codes |
| D6 | Hand-written argument parsing | `main` | Every new flag adds more `if/else` |
| D7 | Magic numbers (5 retries, 10,000 suffixes) | `execute_moves`, `resolve_conflict` | Hard to discover |
| D8 | `current_exe()` + `canonicalize()` for every file | `is_self_executable` | Needless system calls |
| D9 | **Bug:** non-UTF-8 names get corrupted when renamed on conflict | `resolve_conflict` (`to_string_lossy`) | `\xff.txt` becomes `�_1.txt` |
| D10 | `stdout().flush().unwrap()` panics on a broken pipe | `ask_for_directory`, `confirm` | Crash in scripts |
| D11 | Tests use temporary folders with fixed names | `tests::temp_dir` | Parallel runs clash; nothing is cleaned up on failure |
| D12 | `LICENSE` missing although the README links it; `Cargo.lock` ignored in a binary | root, `.gitignore` | Hygiene and reproducible builds |
| D13 | `Cargo.toml` without `license`/`readme`/`keywords`/`categories` | `Cargo.toml` | Blocks publishing to crates.io |
| D14 | No CI | — | The code depends on per-platform behavior and isn't tested on each platform |

---

## Phase 1: Repository hygiene

**Fixes:** D12, D13, D14

- [x] Add `LICENSE` (MIT).
- [x] Remove `Cargo.lock` from `.gitignore` and commit it (binary crate).
- [x] `Cargo.toml` metadata: `license`, `readme`, `keywords`, `categories`.
- [x] `repository`, `homepage` and the README clone URL, once the GitHub
      repository existed.
- [x] CI in `.github/workflows/ci.yml` with a Windows, Linux and macOS
      matrix:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - a build with the declared MSRV (1.82) on Linux and Windows

**Acceptance criterion:** CI passes on all three platforms.

> **MSRV: 1.82.** With the current `Cargo.lock`, `chrono` → `iana-time-zone`
> → `windows-core` 0.62 requires Rust 1.82 on Windows. We chose to raise
> `rust-version` rather than pin old dependencies in the lock (fragile under
> `cargo update`). `.cargo/config.toml` makes Cargo prefer MSRV-compatible
> versions, and the MSRV job runs on Linux and Windows.

---

## Phase 2: Split `lib` and `bin`, with an event-based core

**Fixes:** D1, D2, D11

### Structure

```text
src/
├── lib.rs          # Public core API (re-exports)
├── grouping.rs     # date → folder name (folder_for)
├── plan.rs         # scan() → Plan { moves, warnings }
├── executor.rs     # execute(plan, on_event) → Summary
├── event.rs        # Event, Failure: what the core reports
├── fs_ops.rs       # move_file, copy_then_remove, resolve_conflict (FS safety)
├── main.rs         # CLI entry point → ExitCode
└── cli/
    ├── mod.rs      # organize_directory flow + interactive loop
    ├── args.rs     # argument parsing
    ├── prompt.rs   # confirm, ask_for_directory (stdin)
    └── render.rs   # banner, preview, Event → text, summary
```

### Core API

```rust
pub fn scan(dir: &Path, options: &Options) -> io::Result<Plan>;
pub fn execute(plan: &Plan, on_event: &mut dyn FnMut(Event)) -> Summary;

pub struct Plan        { pub base: PathBuf, pub moves: Vec<PlannedMove>, pub warnings: Vec<ScanWarning> }
pub struct PlannedMove { pub source: PathBuf, pub folder_name: String }
pub enum   ScanWarning { UnreadableEntry(io::Error), UnreadableDir { path, error }, Symlink(PathBuf), NoDate { path, source, error } }

pub enum Event {
    FolderCreated { folder_name },
    FolderRemoved { folder_name },             // undo only
    Conflict { original, renamed_to, late },   // late = appeared after the check (TOCTOU)
    Moved { from, to },
    SourceMissing { path },
    Failed { source, failure: Failure },
}
pub struct Summary { pub moved: u32, pub skipped: u32, pub errors: u32 }
```

### Decisions

- **The `Plan` comes sorted** by destination folder and name. A GUI shows it
  as is, and the CLI groups the preview without re-sorting.
- **Scan warnings are data (`ScanWarning`), not text.** The CLI prints them
  on the same stream as before (symlinks to stdout, the rest to stderr). A GUI
  can list them in a panel.
- **Events are *owned*** (`PathBuf`/`OsString`), so they can be sent through
  a channel (`mpsc`) from a worker thread to the GUI thread. The cost of
  cloning is negligible next to disk operations.
- **`fs_ops` is not exported.** It's the delicate part (never overwrite,
  TOCTOU, keep the mtime) and stays encapsulated with its tests.
- **Tests use `tempfile::TempDir`** (dev-dependency): unique folders that
  clean themselves up.

**Acceptance criterion:** `grep println src/{lib,plan,executor,event,fs_ops,grouping}.rs`
returns nothing, the CLI output is identical and the tests pass.

---

## Phase 3: Executor clarity and minor bugs

**Fixes:** D3, D4, D5, D7, D8, D9, D10

- [x] Split `execute_moves` into:
  - `ensure_dest_dir(base, folder) -> Result<PathBuf, Failure>` (checks it
    doesn't exist as a file and creates the folder, emitting
    `FolderCreated`);
  - `move_with_retries(source, dest_dir, file_name) -> Result<PathBuf, Failure>`
    (conflict pre-check, retry loop on TOCTOU);
  - the `execute` loop is now about 20 lines, with no labels.
- [x] `Summary` with named fields and `Summary::is_success()`.
- [x] `fn main() -> ExitCode` with `EXIT_OK`/`EXIT_FAILURE`/`EXIT_USAGE`
      constants in a single place.
- [x] Named constants: `MAX_MOVE_ATTEMPTS`, `MAX_CONFLICT_SUFFIX`,
      `MAX_PREVIEW_PER_GROUP`.
- [x] Resolve the executable's own path once per scan, and canonicalize a
      file only if its name matches the executable's.
- [x] **Bug D9:** `resolve_conflict` builds the name with `OsString`, never
      through `to_string_lossy`. Tested with a non-UTF-8 name (`cfg(unix)`)
      and an invalid UTF-16 name (`cfg(windows)`).
- [x] **Bug D10:** `let _ = io::stdout().flush();` instead of `unwrap()`.

---

## Phase 4: `clap` + `Options` + `--by-day` (first feature)

**Fixes:** D6. Proves the architecture accepts features without friction.

- [x] `clap` with `derive` (`src/cli/args.rs`). `PATH`, `--yes/-y` and
      `--help/-h` are kept (and `-V/--version` is added), as are the exit
      codes: clap uses 2 for usage errors and 0 for help.
  - The "wrap the path in quotes" hint is kept when there is an extra
    positional argument.
- [x] `pub struct Options { granularity }` (`src/options.rs`), passed to
      `scan`. `Default` keeps the previous behavior.
- [x] `pub enum Granularity { Year, Month, Day }`, defaulting to `Month`, and
      `folder_for(dt, granularity)`:
  - `Year`: `Y2024`
  - `Month`: `Y2024-M01`
  - `Day`: `Y2024-M01-D15`
- [x] `--group-by <year|month|day>` and its `--by-day` shortcut (mutually
      exclusive). The CLI has its own `GroupBy` (`ValueEnum`), so the core
      doesn't depend on clap.
- [x] Tests: `folder_for` for each granularity, `scan` with `Day`, and
      argument parsing (`--yes` without a path, two paths, `--by-day` vs
      `--group-by` conflict).

## Phase 5: More features on top of `Options`

Each one touched a single point of the core (`scan` and `Options`) plus its
flag in `src/cli/args.rs`; `executor`, `fs_ops` and `event` didn't change.

- [x] **`--extensions jpg,png` / `-e`**: `Options::extensions: Option<ExtensionFilter>`
      (`src/filter.rs`). Normalizes `.JPG` → `jpg`, case-insensitive; files
      without an extension never match. Filtered-out files are skipped
      silently.
- [x] **`--recursive` / `-r`**: `Options::recursive`. `scan` became a walk
      (`Scanner::visit`) that:
  - doesn't follow symbolic links or junctions (no cycles), and warns about
    them;
  - skips any folder shaped like a destination (`Y####`, `Y####-M##`,
    `Y####-M##-D##`, `grouping::is_date_folder`) at any depth, so running it
    again changes nothing;
  - moves files into the date folders **of the base folder** (flattening) and
    leaves emptied subfolders in place;
  - records subfolders that can't be listed as a warning
    (`ScanWarning::UnreadableDir`); only failing to list the base folder
    aborts.
  - The preview shows paths relative to the base, to tell same-named files
    apart. Without `-r` it still shows just the name.
- [x] **`--date modified|created`**: `Options::date_source: DateSource`. If
      the platform or filesystem has no creation date, the file is skipped
      with `ScanWarning::NoDate` (formerly `NoModifiedDate`).
  - Note: the copy+delete fallback (across volumes) keeps the mtime but not
    the creation date. Irrelevant within a single base folder.
- [ ] **EXIF** (photo capture date): would be one more `DateSource::Exif`,
      using the `kamadak-exif` dependency. Deferred until needed.
- [x] **Log file**: done in phase 6, since it shares its format and
      mechanism with the journal.

## Phase 6: Move journal, `--undo` and log file

Core in `src/journal.rs`; the CLI only wires up the flags.

- [x] **Journal** at `<base>/.file-organizer-journal.json` (`serde` +
      `serde_json`). Paths relative to the base (still valid if the folder
      moves), plus `version` and `created_at`.
  - `JournalRecorder` builds it by observing `execute`'s `Event`s (`Moved`
    and `FolderCreated`), so `execute` didn't change. Any frontend uses it
    the same way: `recorder.record(&event)`, then `recorder.save()`.
  - It's only saved if the run moved something: a pass with no changes
    doesn't erase what can be undone.
  - Atomic write (temporary file + `rename`).
  - Non-UTF-8 paths don't fit in JSON: they aren't recorded, and the CLI
    warns that those moves can't be undone.
- [x] **`undo(base, &journal, on_event) -> UndoResult { summary, pending }`**:
  - puts files back in reverse order, reusing
    `executor::move_with_retries`, which never overwrites and resolves
    conflicts with a suffix;
  - recreates source subfolders if they were deleted (recursive mode);
  - removes only the created folders that were left empty (`remove_dir`);
    new `Event::FolderRemoved`;
  - whatever fails stays in `pending`; `Journal::persist` saves it for a
    retry, or deletes the journal when nothing is left.
- [x] **Security:** `Journal::load` rejects absolute paths and paths with
      `..`. The journal is an editable file and must not allow moving
      anything outside the base.
- [x] The scan ignores the journal (also in recursive mode).
- [x] **CLI:** `--undo` (requires PATH; incompatible with the scan options)
      with a preview and confirmation (or `-y`), and the "To undo this run…"
      hint after organizing.
- [x] **`--log <FILE>`** (`src/cli/log.rs`): append mode, with the same lines
      as the console plus a timestamp, a header per operation and a result
      line.
  - For this, `render` now produces text (`event_line`, `warning_line` →
    `Line { text, to_stderr }`), which is either printed or written to the
    log.
- [x] Tests: full round trip in recursive mode (including a deleted
      subfolder), non-empty folders that are kept, a journal that survives a
      pass with no changes, rejection of `../`, a folder without a journal
      and `--undo` parsing.

## Phase 7: Workspace + GUI (future work)

**Decision:** the GUI is future work, on its own branch (`feature/gui`). This
phase isn't implemented in `refactor/core-lib`.

The full design is in **[`GUI_DESIGN.md`](GUI_DESIGN.md)**: goals, usability
principles, architecture (workspace, threads, state machine), screens,
accessibility, packaging, testing and milestones G0 to G7.

Compared with what this plan originally expected, the design analysis
simplified the core changes:

- **Progress:** no need for `Event::Progress`. Each move produces exactly one
  terminal event, so the GUI counts events.
- **Running a subset:** no new API needed. `Plan::moves` is public, and
  `#[derive(Clone)]` on `PlannedMove` is enough.
- **Still needed:** cooperative cancellation (`execute_cancellable` /
  `undo_cancellable`), `date`/`size` on `PlannedMove`, and `Display` for
  `Failure`/`ScanWarning`.

## English localization

All user-facing text (CLI messages, prompts, `--help`), code comments and
documentation are in English. Prompts accept `y`/`yes` and `n`/`no` (plus
`q`/`quit`/`exit`). With English text, `--help` uses clap's standard layout,
so the custom help template from phase 4 was removed.

---

## Status

| Phase | Status |
|-------|--------|
| 1. Hygiene | ✅ · MSRV 1.82 |
| 2. lib/bin + events | ✅ |
| 3. Clarity + bugs | ✅ |
| 4. clap + `--by-day` | ✅ |
| 5. Features on `Options` | ✅ (EXIF deferred; log → phase 6) |
| 6. Journal + `--undo` + log | ✅ |
| 7. Workspace + GUI | 📄 Designed in `GUI_DESIGN.md`; to be built on `feature/gui` |
