# GUI design (future work)

> **Status:** design approved, not implemented. It will be built on its own
> branch (proposed: `feature/gui`) off `main`, once `refactor/core-lib` is
> merged.
>
> **Prerequisite:** the core is already prepared (phases 1 to 6 of
> [`REFACTOR_PLAN.md`](REFACTOR_PLAN.md)). `scan`, `execute`, `undo`,
> `Options`, `Event` and `Journal` never print or read stdin, so the GUI uses
> them directly.

## Contents

1. [Goals and user](#1-goals-and-user)
2. [Usability principles](#2-usability-principles)
3. [Technology](#3-technology)
4. [Architecture](#4-architecture)
5. [Required core changes](#5-required-core-changes)
6. [Flow and screens](#6-flow-and-screens)
7. [Component details](#7-component-details)
8. [States, errors and edge cases](#8-states-errors-and-edge-cases)
9. [Accessibility and language](#9-accessibility-and-language)
10. [Persisted preferences](#10-persisted-preferences)
11. [Packaging and distribution](#11-packaging-and-distribution)
12. [Testing strategy](#12-testing-strategy)
13. [Milestones](#13-milestones)
14. [Acceptance criteria](#14-acceptance-criteria)
15. [Risks and open decisions](#15-risks-and-open-decisions)

---

## 1. Goals and user

**Who it's for:** someone who doesn't use the terminal and wants to tidy up a
messy folder (Downloads, phone photos, desktop) without fear of losing files.

**What they must be able to do, in order of importance:**

1. Pick a folder, see **what will happen** and confirm it, in under 30
   seconds and without reading any documentation.
2. **Undo** with one click if they don't like the result.
3. Tweak common options (by year, month or day; photos only; include
   subfolders) without understanding technical concepts.
4. Exclude specific files before running.

**Out of scope for the first version:** custom rules, several folders at
once, scheduled runs, watching folders in the background, and EXIF.

---

## 2. Usability principles

| # | Principle | How it's applied |
|---|-----------|------------------|
| P1 | **Nothing moves without a preview and an explicit click** | The "Organize" button is only enabled with a computed, visible plan. There is no "don't ask" mode in the GUI |
| P2 | **You can always go back** | After organizing, "Undo" takes the place of the main button. Opening a folder that has a journal offers to undo the previous run |
| P3 | **Safe, useful defaults** | Month, modification date, no recursion and all file types: the same as the CLI with no flags |
| P4 | **Plain language** | "Include subfolders" instead of "recursive"; "Date created" instead of "ctime"; never `Y2024-M01` without an example next to it |
| P5 | **Immediate feedback** | Changing an option recomputes the preview (debounced). Long operations show progress and can be cancelled |
| P6 | **Errors are explained and actionable** | Every error says what happened, which file it affects and what the user can do. No raw OS error codes without context |
| P7 | **One window, a linear flow** | Pick, review, organize, then see the result or undo. No tabs or nested menus |
| P8 | **The UI never freezes** | Scanning, moving and undoing run on a worker thread |

---

## 3. Technology

**Decision: `eframe`/`egui`**, the latest stable version when the branch is
created.

| Criterion | egui (chosen) | Tauri (alternative) |
|-----------|---------------|---------------------|
| Language | 100% Rust | Rust + HTML/CSS/JS |
| Binary | Single, no runtime | Needs WebView2 (Windows) or WebKitGTK |
| Core integration | Direct calls | Serialized IPC commands |
| Look | Functional, not native | Anything (web) |
| Maintenance cost | Low | Two ecosystems |

**Supporting crates:**

| Crate | Use |
|-------|-----|
| `eframe` (+ `persistence` feature) | Window, event loop and saved preferences |
| `egui_extras` | `TableBuilder` with virtualized rows (thousands of files) |
| `rfd` | Native "Select folder" dialog |
| `opener` | "Open folder in file explorer" when done |
| `egui_kittest` (dev) | UI interaction tests |

> **MSRV:** `eframe` usually requires a newer Rust than the core (currently
> 1.82). That's why the GUI lives in its own crate with its own
> `rust-version`: the core and the CLI keep theirs.

---

## 4. Architecture

### 4.1 Workspace

First step on the branch: turn the repo into a workspace **without changing
the CLI's behavior**.

```text
Cargo.toml                    # [workspace] members = ["crates/*"]
crates/
├── organizer-core/           # src/{lib,plan,executor,event,fs_ops,grouping,
│                             #      options,filter,journal}.rs (moved as is)
├── organizer-cli/            # src/{main.rs, cli/}  → `file-organizer-by-date` binary
└── organizer-gui/            # `file-organizer-gui` binary
    └── src/
        ├── main.rs           # eframe startup, icon, minimum size
        ├── app.rs            # impl eframe::App: only draws and dispatches messages
        ├── state.rs          # AppState + Message + update(): pure, testable logic
        ├── worker.rs         # worker thread: scan / execute / undo → channel
        ├── views/
        │   ├── folder.rs     # folder picker + "undo previous run" notice
        │   ├── options.rs    # options panel
        │   ├── preview.rs    # grouped table with checkboxes
        │   ├── progress.rs   # progress bar + live log + cancel
        │   └── result.rs     # summary + undo + open folder
        └── text.rs           # every visible string (basis for i18n)
```

- **The CLI package name stays** `file-organizer-by-date`, so existing
  installs and scripts keep working.
- **CI:** add `organizer-gui` to the matrix. On Linux it needs eframe's
  system dependencies, and its own job if its MSRV differs.

### 4.2 Threading model

```text
┌────────────── UI thread (egui) ─────────┐        ┌───── worker thread ──────┐
│ AppState ──Message──► update()          │        │                          │
│     ▲                    │ Command      │  cmd   │ scan(dir, &options)      │
│     │                    └──────────────┼───────►│ execute(plan, on_event,  │
│     │                                   │        │         &cancel)         │
│     │   WorkerMsg (Plan, Event,         │  msgs  │ undo(base, &journal, …)  │
│     └── Summary, UndoResult, error) ◄───┼────────│                          │
│         + ctx.request_repaint()         │        └──────────────────────────┘
└─────────────────────────────────────────┘
```

- **A single long-lived worker thread** receives `Command`s over
  `std::sync::mpsc`. That's enough, because the app never runs two disk
  operations at once.
- **The worker sends each `Event` over the channel** and calls
  `egui::Context::request_repaint()` so the UI wakes up.
- **Cancellation:** a shared `Arc<AtomicBool>` (see §5.2).
- **Re-scans when options change:** each `Command::Scan` carries a
  `generation: u64`. The UI drops `Plan`s from old generations, with no locks
  needed.

### 4.3 State as a state machine

```rust
enum Screen {
    PickFolder,                                   // no folder yet
    Scanning { generation: u64 },
    Review { plan: Plan, selection: Vec<bool> },  // editable preview
    Running { total: usize, done: usize, log: Vec<LogLine> },
    Done { summary: Summary, journal_saved: bool },
    Undoing { total: usize, done: usize, log: Vec<LogLine> },
    Undone { summary: Summary },
}

struct AppState {
    folder: Option<PathBuf>,
    options: Options,
    previous_run: Option<Journal>,   // Journal::load when a folder is picked
    screen: Screen,
    banner: Option<Banner>,          // non-blocking errors/warnings
}
```

`update(state, msg) -> Vec<Command>` is a **pure function**: all the flow
logic is tested without opening a window (§12). `app.rs` only draws `state`
and turns clicks into `Message`s.

---

## 5. Required core changes

They're small and backward compatible. They go on the GUI branch with their
tests, and the CLI benefits where it makes sense.

### 5.1 Progress: no changes needed

**Invariant** to document and lock in with a test: every `PlannedMove`
produces exactly one terminal event (`Moved`, `SourceMissing` or `Failed`),
and so does every `JournalMove` in `undo`. The GUI computes
`done / plan.moves.len()` by counting those events, with no `Event::Progress`.

### 5.2 Cooperative cancellation (needed)

```rust
pub fn execute_cancellable(
    plan: &Plan,
    on_event: &mut dyn FnMut(Event),
    cancel: &AtomicBool,
) -> Summary;            // Summary gains `cancelled: bool`
```

- **`cancel` is checked between files**, never in the middle of a move: a
  file is either moved or not, never half-moved.
- **`execute` delegates** to `execute_cancellable` with an `AtomicBool` set
  to `false`, so the current API doesn't change.
- **The journal records what did move:** "Undo" also works after a
  cancellation. This needs a test.
- Same for `undo` (`undo_cancellable`).

### 5.3 Running only the selection: no changes needed

`Plan::moves` is public. The GUI builds a filtered `Plan`:

```rust
let selected = Plan {
    base: plan.base.clone(),
    moves: plan.moves.iter().zip(&selection).filter(|(_, s)| **s).map(|(m, _)| m.clone()).collect(),
    warnings: Vec::new(),
};
```

This needs `#[derive(Clone)]` on `PlannedMove`, a trivial change.

### 5.4 Data for the table (recommended)

- **`PlannedMove::date: DateTime<Local>`**: the date used for grouping, so
  the table can show it without reading metadata again.
- **`PlannedMove::size: u64`**: the size, for totals like "1.2 GB in 340
  files".
- **Optional: `Plan::predicted_conflicts()`.** Flags files whose name already
  exists at the destination, to show "will be renamed" before running. It's
  informational only: the executor still resolves the real conflicts.

### 5.5 Readable error messages

Implement `Display` for `Failure` and `ScanWarning` in the core (today the
text lives in `cli/render.rs`). The GUI and the CLI then share the base
wording, and the CLI only adds icons and colors.

---

## 6. Flow and screens

```text
 ┌─────────────┐  folder    ┌──────────┐  plan   ┌──────────┐  Organize   ┌─────────┐  end  ┌───────────┐
 │ PickFolder  │──────────► │ Scanning │───────► │  Review  │───────────► │ Running │─────► │   Done    │
 └─────────────┘            └──────────┘         └──────────┘             └─────────┘       └───────────┘
        ▲                         ▲  option changed   │  │                     │ Cancel          │ Undo
        │                         └───────────────────┘  │ "Change folder"     ▼                 ▼
        └────────────────────────────────────────────────┘                  Done (partial)   Undoing ─► Undone
```

### 6.1 Main window (Review screen)

Minimum window size 900×600, resizable. The options panel is always visible
on the left; the content area changes with the screen.

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│  📂 File Organizer by Date                                                   │
├──────────────────────────────────────────────────────────────────────────────┤
│  Folder: [ C:\Users\ana\Downloads                          ] [Change…]       │
│  ↩️ This folder was organized on 2026-10-20 14:03 (312 files).   [Undo]      │
├────────────────────────┬─────────────────────────────────────────────────────┤
│ OPTIONS                │  Preview — 340 files · 1.2 GB · 14 folders          │
│                        │  [🔍 Search file…         ]  [All] [None]           │
│ Group by               │ ┌─────────────────────────────────────────────────┐ │
│  ○ Year     2024       │ │☑ ▼ 📁 2024 · January (Y2024-M01)      23 files  │ │
│  ● Month    2024-M01   │ │   ☑ beach_photo.jpg       2024-01-15   2.1 MB   │ │
│  ○ Day   2024-M01-D15  │ │   ☑ report.pdf            2024-01-20   340 KB   │ │
│                        │ │   ☐ invoice.pdf  ⚠ will be renamed     12 KB    │ │
│ Use date               │ │☑ ▶ 📁 2024 · March (Y2024-M03)        41 files  │ │
│  ● Modified            │ │◪ ▶ 📁 2025 · November (Y2025-M11)      8 files  │ │
│  ○ Created             │ │                    …                            │ │
│                        │ └─────────────────────────────────────────────────┘ │
│ File types             │  ⚠ 2 items skipped (links, no date)  [Show]        │
│  ● All                 │                                                     │
│  ○ Only: [Photos ▾]    │                                                     │
│    jpg, png, heic …    │                                                     │
│                        │                                                     │
│ ☐ Include subfolders   │         [ Cancel ]   [ ✔ Organize 339 files ]       │
└────────────────────────┴─────────────────────────────────────────────────────┘
```

### 6.2 Progress (Running / Undoing)

```text
│  Organizing… 128 of 339                                                      │
│  [██████████████░░░░░░░░░░░░░░░░░░░░░░░░]  38 %                              │
│  ✔ beach_photo.jpg → 2024 · January                                          │
│  ⚠ invoice.pdf already existed → invoice_1.pdf                               │
│  ✔ report.pdf → 2024 · January                                               │
│                                                       [ Cancel ]             │
```

### 6.3 Result (Done)

```text
│  ✅ Done: 337 files organized into 14 folders.                               │
│     1 renamed to avoid overwriting · 0 skipped · 2 errors  [Show details]    │
│                                                                              │
│     [ 📂 Open folder ]   [ ↩️ Undo ]   [ Organize another folder ]           │
```

If there were errors, the summary is shown in amber and "Show details"
expands the list. Each error shows the file, the reason in plain language and
a suggestion: "Close it if it's open in another program and press Retry".

---

## 7. Component details

### 7.1 Folder picker

- There are three ways to pick it:
  - the "Choose folder…" button, with `rfd`'s native dialog;
  - **dragging and dropping** a folder onto the window (`egui` exposes
    `dropped_files`);
  - pasting or typing the path, validated on Enter.
- The initial screen (PickFolder) is a large drop zone reading "Drop a folder
  here or press Choose folder".
- **When a folder is picked:**
  - `Journal::load` shows, if there is one, the notice "↩️ This folder was
    organized on … [Undo]";
  - a `Scan` starts automatically, with no "Analyze" button.
- **Safety:** before scanning the root of a drive (`C:\`, `/`) or the whole
  user folder, ask for an extra confirmation: "This folder contains many
  system files…".

### 7.2 Options panel

Each control maps exactly to an `Options` field:

| Control | Field | UX details |
|---------|-------|------------|
| Group by: Year / Month / Day | `granularity` | Each option shows a live example with today's date |
| Use date: Modified / Created | `date_source` | Tooltip: "The creation date may not exist on some drives; those files will be skipped" |
| File types: All / Only… | `extensions` | Presets (Photos, Videos, Documents, Music, Archives) plus a free field of comma-separated extensions. Presets are data in `text.rs`, not logic |
| Include subfolders | `recursive` | Help text: "Files in subfolders will be moved into this folder's date folders. Already-organized folders are skipped" |

- **Changes trigger a re-scan**, debounced by about 300 ms.
- **The user's manual selection is kept** across re-scans as long as the file
  is still in the plan (indexed by `source`).

### 7.3 Preview table

- **Virtualized rows** with `egui_extras::TableBuilder`, so 50,000 rows don't
  slow the UI down.
- **Grouped by destination folder**, taking advantage of the `Plan` being
  sorted by `folder_name`. Groups are collapsible, tri-state (☑ ◪ ☐) and show
  the file count.
- **Folder names are shown in a friendly form** ("2024 · January"), with the
  technical name (`Y2024-M01`) in parentheses or a tooltip.
- **Columns:** checkbox, file (path relative to the base), date used, size,
  and status ("⚠ will be renamed" if §5.4 is implemented).
- **Search** by name, plus All / None buttons.
- **The main button reflects the selection:** "Organize 339 files". With 0
  selected it's disabled.
- **Scan warnings (`ScanWarning`) are summarized in one line** with a "Show"
  link that opens a panel with the list. They never block.

### 7.4 Running

- **Organize:**
  - builds the filtered `Plan` (§5.3);
  - sends `Command::Execute`, with a `JournalRecorder` inside the worker,
    exactly like the CLI does;
  - disables the options panel while running.
- **The live log** shows the last N events with auto-scroll; the full log can
  be copied to the clipboard.
- **Cancel** sets the `AtomicBool`. The button changes to "Cancelling…" until
  the current file finishes, and the result says "Cancelled: 128 of 339
  moved (can be undone)".

### 7.5 Undo

- **Available from two places:** the result of a run that just finished, and
  the "previous run" notice when opening a folder.
- **Before undoing**, a modal dialog shows "337 files will be returned to
  their original location" and a summarized list (`journal.moves`).
- **After undoing:**
  - if `pending` isn't empty, report "3 files couldn't be returned" and offer
    "Retry";
  - `Journal::persist` takes care of the file, just like in the CLI.

### 7.6 Window and system

- Closing during an operation: "An operation is in progress. Cancel it and
  quit?". Cancelling is safe (§5.2).
- Keyboard shortcuts:
  - `Ctrl+O`: choose folder;
  - `Enter`: main action;
  - `Ctrl+Z`: undo, only on the result screen and always with the modal
    confirmation;
  - `Esc`: cancel or close the dialog.

---

## 8. States, errors and edge cases

| Situation | Behavior |
|-----------|----------|
| Folder with nothing to organize | Illustration and text: "There are no loose files here". If `recursive` is off, suggest turning it on |
| Error listing the folder (permissions) | Red banner with the cause and a "Choose another folder" button |
| Folder deleted or unmounted between scan and run | Every file produces `SourceMissing`. The summary explains it and it doesn't count as an error |
| File in use (Windows) | `Failed` with an actionable message and "Retry" for the failed ones (a new `Plan` with only those) |
| Corrupt journal or invalid paths in it | The notice "Can't undo: the record is damaged" offers "Show file". It's never deleted automatically |
| Moves not recorded (non-UTF-8 paths) | Persistent notice in the result: "N files can't be undone" |
| Very long scan | Spinner with a count of files seen, and "Cancel". Requires `scan` to support cancellation; otherwise the result is dropped by `generation` |
| Options changed during a scan | A new scan starts and the old plan is dropped by `generation` |

---

## 9. Accessibility and language

- **Screen readers:** egui integrates AccessKit. Every control needs a real
  label: no icon-only controls.
- **Color is never the only signal:** errors, warnings and successes carry an
  icon and text besides color.
- **Sufficient contrast** in both light and dark themes (egui ships both);
  follows the system theme by default.
- **Adjustable UI scale** (`Ctrl +` / `Ctrl -`) via `pixels_per_point`.
- **Keyboard:** the whole flow can be completed without a mouse, with a
  logical tab order.
- **Language:**
  - every string lives in `text.rs` from day one;
  - the first version ships in English, like the CLI;
  - if more languages are needed later, `text.rs` is migrated to `fluent`
    or `rust-i18n` without touching the views.

---

## 10. Persisted preferences

Saved with `eframe::Storage` (`persistence` feature):

- the last folder used (only suggested; never scanned at startup without a
  user action);
- the last `Options`;
- window size and position, theme and scale.

Nothing about the files is persisted: the journal in each folder remains the
only source of truth for undo, and it's compatible with the CLI.

---

## 11. Packaging and distribution

- **Windows:**
  - `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`, so
    no console window opens;
  - embedded icon (e.g. with `winres`);
  - code signing, if distributed publicly.
- **macOS:** an `.app` bundle (e.g. with `cargo-bundle`).
- **Linux:** AppImage or `.deb`.
- **Pipeline:** `cargo-dist` can generate installers and GitHub releases for
  both the CLI and the GUI from the same workspace.
- **The CLI and the GUI ship separately.** The CLI must not pull in the
  graphics dependencies.

---

## 12. Testing strategy

1. **Core:** unit tests for the §5 changes:
   - cancellation between files;
   - partial journal after cancelling, and that it can be undone;
   - the one-terminal-event-per-move invariant.
2. **App logic:** tests of `update(state, msg)` with no window:
   - valid transitions;
   - plans dropped by `generation`;
   - selection kept across re-scans;
   - "Organize" disabled with no selection.
3. **Interaction:** `egui_kittest` to walk through the main flow (pick a
   folder, uncheck a group, organize, undo) on a `tempdir`.
4. **Manual, before each release**, on Windows, macOS and Linux:
   - a folder with 10,000 files;
   - a file open in another program;
   - a folder on a network drive;
   - cancelling halfway;
   - closing the window during an operation.

---

## 13. Milestones

Branch: `feature/gui`, with one PR per milestone to keep reviews small.

| Milestone | Content | Verifiable outcome |
|-----------|---------|--------------------|
| G0 | Workspace `crates/{core,cli,gui}`, extended CI | The CLI behaves the same: the output comparison script shows no differences |
| G1 | Core: cancellation, `Clone` and `date`/`size` on `PlannedMove`, `Display` for errors | New tests pass. The CLI uses `Display` without changing its output |
| G2 | GUI skeleton: `AppState` + `update` + worker + folder picker + scan | Picking a folder shows the file count without freezing the UI |
| G3 | Options panel and preview table with selection | Changing options re-scans; unchecking files updates the button |
| G4 | Running with progress, live log, cancellation and result | Organizing and cancelling work, and the journal is compatible with the CLI's `--undo` |
| G5 | Undo (from the result and the notice when opening a folder), retry failed moves | Full round trip from the GUI |
| G6 | Accessibility, shortcuts, persistence, safety checks (drive root), copy polish | §9 and §14 checklists complete |
| G7 | Per-platform packaging and documentation (README with screenshots) | Installers downloadable from a release |

---

## 14. Acceptance criteria

- [ ] A new user organizes a folder and undoes it **without help** in under a
      minute (tested with 3 people).
- [ ] Nothing moves without a preview and an explicit click (P1).
- [ ] The UI doesn't freeze with 50,000 files; scanning and running show
      progress.
- [ ] Cancelling leaves a consistent, undoable state.
- [ ] A journal created by the GUI can be undone with the CLI, and vice
      versa.
- [ ] The whole flow can be completed with the keyboard alone; controls have
      labels for screen readers.
- [ ] The CLI's behavior and dependencies don't change.
- [ ] CI passes on Windows, macOS and Linux.

---

## 15. Risks and open decisions

| Topic | Risk / question | Proposal |
|-------|-----------------|----------|
| eframe MSRV | Higher than the core's | Separate `rust-version` in `organizer-gui` |
| egui's non-native look | May feel "unpolished" | A careful theme, consistent icons and generous spacing. If it becomes a problem, the UI-free core makes moving to Tauri possible |
| Huge scans | `scan` can't be cancelled today | Add `cancel` to `scan` in G1 if G2's testing justifies it |
| Hidden journal on Windows | The leading dot doesn't hide the file | Optional: set the `HIDDEN` attribute on creation (replacing a hidden file on Windows needs care) |
| Languages | Is anything besides English needed in the first version? | No; `text.rs` keeps the door open |
| OS integration | "Organize with…" in the file explorer's context menu | Out of scope; possible v2 |
