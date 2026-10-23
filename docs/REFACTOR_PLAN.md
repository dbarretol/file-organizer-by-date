# Plan de refactor: núcleo reutilizable, extensibilidad y GUI futura

> Rama de trabajo: `refactor/core-lib`
> Punto de partida: v0.5.0 (`ff55cb1`). Un único `src/main.rs` de 687 líneas,
> `clippy` limpio y 6 tests en verde.

## Objetivo

1. **Mantenibilidad y claridad:** funciones cortas, tipos con nombre,
   responsabilidades separadas.
2. **Añadir features con poca fricción:** cada feature del roadmap toca un único
   punto de extensión, no tres funciones a la vez.
3. **Preparar una GUI:** el núcleo no imprime ni lee stdin. Devuelve datos
   (`Plan`, `Summary`) y emite eventos (`Event`) que cualquier frontend puede
   consumir.

**Regla transversal:** en las fases 1 a 3 el comportamiento visible de la CLI
no cambia (mismos mensajes, mismos códigos de salida). Los tests existentes
deben seguir pasando en cada fase.

---

## Diagnóstico (resumen de la auditoría)

| # | Problema | Ubicación (v0.5.0) | Impacto |
|---|----------|--------------------|---------|
| D1 | El núcleo imprime por consola (`println!`/`eprintln!`) | `collect_files`, `execute_moves`, `organize_directory` | Una GUI no puede reutilizar la lógica |
| D2 | Todo en un archivo y sin crate `lib` | `src/main.rs` | No hay tests de integración ni reutilización |
| D3 | `execute_moves` ocupa unas 130 líneas, con bucles anidados y `continue 'files` | `src/main.rs:184-312` | Difícil de leer y de modificar |
| D4 | Retorno `(u32, u32, u32)` | `execute_moves` | Hay que recordar el orden |
| D5 | `process::exit` repartido por `main` | `main` | Códigos de salida sin una fuente única |
| D6 | Parseo de argumentos manual | `main` | Cada flag nuevo añade más `if/else` |
| D7 | Números mágicos (5 reintentos, 10 000 sufijos) | `execute_moves`, `resolve_conflict` | Poco descubribles |
| D8 | `current_exe()` + `canonicalize()` por cada archivo | `is_self_executable` | Llamadas al sistema innecesarias |
| D9 | **Bug:** los nombres no UTF-8 se corrompen al renombrar por conflicto | `resolve_conflict` (`to_string_lossy`) | `\xff.txt` pasa a llamarse `�_1.txt` |
| D10 | `stdout().flush().unwrap()` entra en pánico con un pipe roto | `ask_for_directory`, `confirm` | Crash en scripts |
| D11 | Los tests usan directorios temporales con nombre fijo | `tests::temp_dir` | Dos ejecuciones en paralelo chocan; no se limpia si falla |
| D12 | Falta `LICENSE` aunque el README lo enlaza; `Cargo.lock` ignorado en un binario | raíz, `.gitignore` | Higiene y builds reproducibles |
| D13 | `Cargo.toml` sin `license`/`readme`/`keywords`/`categories` | `Cargo.toml` | Bloquea publicar en crates.io |
| D14 | Sin CI | — | El código depende del comportamiento de cada plataforma y no se prueba en ellas |

---

## Fase 1: Higiene del repositorio

**Resuelve:** D12, D13, D14

- [x] Añadir `LICENSE` (MIT).
- [x] Quitar `Cargo.lock` del `.gitignore` y versionarlo (es un binario).
- [x] Metadatos en `Cargo.toml`: `license`, `readme`, `keywords`, `categories`.
- [ ] `repository`, `homepage` y la URL de clonado del README. **Pendiente
      hasta que exista un remoto** (hoy es un placeholder).
- [x] CI en `.github/workflows/ci.yml` con matriz Windows, Linux y macOS:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - build con la MSRV declarada (1.82) en Linux y Windows

**Criterio de aceptación:** el CI pasa en las tres plataformas.

> **MSRV: 1.82.** Con el `Cargo.lock` actual, `chrono` → `iana-time-zone` →
> `windows-core` 0.62 exige Rust 1.82 en Windows. Se optó por subir
> `rust-version` en lugar de fijar dependencias antiguas en el lock (frágil
> ante `cargo update`). El job de MSRV corre en Linux y en Windows.

---

## Fase 2: Separar `lib` y `bin`, con un núcleo basado en eventos

**Resuelve:** D1, D2, D11

### Estructura

```text
src/
├── lib.rs          # API pública del núcleo (re-exports)
├── grouping.rs     # fecha → nombre de carpeta (folder_for)
├── plan.rs         # scan() → Plan { moves, warnings }
├── executor.rs     # execute(plan, on_event) → Summary
├── event.rs        # Event, Failure: lo que el núcleo comunica
├── fs_ops.rs       # move_file, copy_then_remove, resolve_conflict (seguridad FS)
├── main.rs         # punto de entrada de la CLI → ExitCode
└── cli/
    ├── mod.rs      # flujo organize_directory + bucle interactivo
    ├── args.rs     # parseo de argumentos → Command
    ├── prompt.rs   # confirm, ask_for_directory (stdin)
    └── render.rs   # banner, preview, Event → texto, resumen
```

### API del núcleo

```rust
pub fn scan(dir: &Path, options: &Options) -> io::Result<Plan>;
pub fn execute(plan: &Plan, on_event: &mut dyn FnMut(Event)) -> Summary;

pub struct Plan        { pub base: PathBuf, pub moves: Vec<PlannedMove>, pub warnings: Vec<ScanWarning> }
pub struct PlannedMove { pub source: PathBuf, pub folder_name: String }
pub enum   ScanWarning { UnreadableEntry(io::Error), Symlink(PathBuf), NoModifiedDate { path, error } }

pub enum Event {
    FolderCreated { folder_name },
    Conflict { original, renamed_to, late },   // late = apareció tras la comprobación (TOCTOU)
    Moved { from, to },
    SourceMissing { path },
    Failed { source, failure: Failure },
}
pub struct Summary { pub moved: u32, pub skipped: u32, pub errors: u32 }
```

### Decisiones

- **El `Plan` ya viene ordenado** por carpeta destino y nombre. La GUI lo
  muestra tal cual y la CLI agrupa la vista previa sin reordenar.
- **Los avisos del escaneo son datos (`ScanWarning`), no texto.** La CLI los
  imprime en el mismo canal que antes (los symlinks por stdout y el resto por
  stderr). La GUI puede listarlos en un panel.
- **Los eventos son *owned*** (`PathBuf`/`OsString`). Así se pueden enviar
  por un canal (`mpsc`) desde un hilo de trabajo hasta el hilo de la GUI. El
  coste de clonar es despreciable frente al de las operaciones de disco.
- **`fs_ops` no se exporta.** Es la parte delicada (no sobrescribir, TOCTOU,
  conservar el mtime) y queda encapsulada con sus tests.
- **Tests con `tempfile::TempDir`** (dev-dependency): directorios únicos que
  se limpian solos.

**Criterio de aceptación:** `grep println src/{lib,plan,executor,event,fs_ops,grouping}.rs`
no devuelve nada, la salida de la CLI es idéntica y los tests pasan.

---

## Fase 3: Claridad del ejecutor y bugs menores

**Resuelve:** D3, D4, D5, D7, D8, D9, D10

- [x] Trocear `execute_moves` en:
  - `ensure_dest_dir(base, folder) -> Result<PathBuf, Failure>` (comprueba
    que no exista como archivo y crea la carpeta, emitiendo `FolderCreated`).
  - `move_with_retries(source, dest_dir, file_name) -> Result<PathBuf, Failure>`
    (pre-check de conflicto, bucle de reintentos ante TOCTOU).
  - El bucle de `execute` queda en unas 20 líneas, sin etiquetas.
- [x] `Summary` con campos con nombre y `Summary::is_success()`.
- [x] `fn main() -> ExitCode` con constantes `EXIT_OK`/`EXIT_FAILURE`/`EXIT_USAGE`
      en un único sitio.
- [x] Constantes con nombre: `MAX_MOVE_ATTEMPTS`, `MAX_CONFLICT_SUFFIX`,
      `MAX_PREVIEW_PER_GROUP`.
- [x] Calcular la ruta del propio ejecutable una sola vez por escaneo, y
      canonicalizar un archivo solo si su nombre coincide con el del ejecutable.
- [x] **Bug D9:** `resolve_conflict` construye el nombre con `OsString`, sin
      pasar por `to_string_lossy`. Test con un nombre no UTF-8 (`cfg(unix)`)
      y con un nombre no UTF-16 válido (`cfg(windows)`).
- [x] **Bug D10:** `let _ = io::stdout().flush();` en lugar de `unwrap()`.

---

## Fase 4: `clap` + `Options` + `--by-day` (primera feature)

**Resuelve:** D6. Valida que la arquitectura admite features sin fricción.

- [x] `clap` con `derive` (`src/cli/args.rs`). Se mantienen `RUTA`, `--yes/-y`,
      `--help/-h` (y se añade `-V/--version`) y los códigos de salida: clap
      usa el 2 para errores de uso y el 0 para la ayuda.
  - La ayuda está en español (`help_template`, `override_usage` y
    encabezados propios). **Limitación:** los mensajes de error de clap
    ("error: unexpected argument…") siguen en inglés; clap no ofrece i18n.
  - Se conserva la pista de "escriba la ruta entre comillas" cuando sobra un
    argumento posicional.
- [x] `pub struct Options { granularity }` (`src/options.rs`), que se pasa a
      `scan`. `Default` conserva el comportamiento anterior.
- [x] `pub enum Granularity { Year, Month, Day }`, con `Month` por defecto, y
      `folder_for(dt, granularity)`:
  - `Year`: `Y2024`
  - `Month`: `Y2024-M01`
  - `Day`: `Y2024-M01-D15`
- [x] `--group-by <year|month|day>` y su atajo `--by-day` (en conflicto entre
      sí). La CLI tiene su propio `GroupBy` (`ValueEnum`), de modo que el
      núcleo no depende de clap.
- [x] Tests: `folder_for` en cada granularidad, `scan` con `Day` y el parseo
      de argumentos (`--yes` sin ruta, dos rutas, conflicto entre
      `--by-day` y `--group-by`).

## Fase 5: Más features sobre `Options`

Cada una tocó un único punto del núcleo (`scan` y `Options`) más su flag en
`src/cli/args.rs`; `executor`, `fs_ops` y `event` no cambiaron.

- [x] **`--extensions jpg,png` / `-e`**: `Options::extensions: Option<ExtensionFilter>`
      (`src/filter.rs`). Normaliza `.JPG` → `jpg`, sin distinguir mayúsculas;
      los archivos sin extensión nunca coinciden. Los archivos filtrados se
      omiten en silencio.
- [x] **`--recursive` / `-r`**: `Options::recursive`. `scan` pasa a ser un
      recorrido (`Scanner::visit`) que:
  - no sigue enlaces simbólicos ni junctions (sin ciclos), y avisa de ellos;
  - salta cualquier carpeta con forma de destino (`Y####`, `Y####-M##`,
    `Y####-M##-D##`, `grouping::is_date_folder`) a cualquier profundidad, de
    modo que repetir la ejecución no cambia nada;
  - mueve a las carpetas por fecha **de la carpeta base** (aplana) y deja las
    subcarpetas vaciadas en su sitio;
  - registra como aviso (`ScanWarning::UnreadableDir`) las subcarpetas que no
    se pueden listar; solo un fallo al listar la carpeta base aborta.
  - La vista previa muestra rutas relativas a la base, para distinguir
    homónimos. Sin `-r` sigue mostrando solo el nombre.
- [x] **`--date modified|created`**: `Options::date_source: DateSource`.
      Si la plataforma o el sistema de archivos no ofrece fecha de creación,
      el archivo se omite con `ScanWarning::NoDate` (antes `NoModifiedDate`).
  - Nota: el fallback copiar+borrar (entre volúmenes) conserva el mtime pero
    no la fecha de creación. Es irrelevante dentro de una misma carpeta base.
- [ ] **EXIF** (fecha de captura de fotos): sería un `DateSource::Exif` más,
      con la dependencia `kamadak-exif`. Queda pendiente hasta que haga falta.
- [ ] **Log a archivo**: pasa a la fase 6, porque comparte formato y
      mecanismo con el journal.

## Fase 6: Journal de movimientos, `--undo` y log a archivo

Núcleo en `src/journal.rs`; la CLI solo conecta los flags.

- [x] **Journal** en `<base>/.file-organizer-journal.json` (`serde` +
      `serde_json`). Rutas relativas a la base (sigue siendo válido si se
      mueve la carpeta), `version` y `created_at`.
  - `JournalRecorder` lo construye observando los `Event` de `execute`
    (`Moved` y `FolderCreated`), así que `execute` no cambió. Cualquier
    frontend lo usa igual: `recorder.record(&event)` y luego `recorder.save()`.
  - Solo se guarda si la ejecución movió algo: una pasada sin cambios no
    borra lo que se puede deshacer.
  - Escritura atómica (temporal + `rename`).
  - Las rutas que no son UTF-8 no caben en JSON: no se registran, y la CLI
    avisa de que esos movimientos no se podrán deshacer.
- [x] **`undo(base, &journal, on_event) -> UndoResult { summary, pending }`**:
  - devuelve los archivos en orden inverso reutilizando
    `executor::move_with_retries`, que nunca sobrescribe y resuelve
    conflictos con sufijo;
  - recrea las subcarpetas de origen si se borraron (modo recursivo);
  - elimina solo las carpetas creadas que quedaron vacías (`remove_dir`);
    nuevo `Event::FolderRemoved`;
  - lo que falla queda en `pending`; `Journal::persist` lo guarda para
    reintentarlo o borra el journal si ya no queda nada.
- [x] **Seguridad:** `Journal::load` rechaza rutas absolutas o con `..`. El
      journal es un archivo editable y no debe permitir mover nada fuera de la
      base.
- [x] El escaneo ignora el journal (también en modo recursivo).
- [x] **CLI:** `--undo` (requiere RUTA; incompatible con las opciones de
      escaneo) con vista previa y confirmación (o `-y`), y la pista
      "Puede deshacerlo con…" tras organizar.
- [x] **`--log <ARCHIVO>`** (`src/cli/log.rs`): en modo append, con las mismas
      líneas que la consola más una marca de tiempo, una cabecera por operación
      y una línea de resultado.
  - Para ello `render` pasó a producir texto (`event_line`, `warning_line`
    → `Line { text, to_stderr }`), que se imprime o se escribe en el log.
- [x] Tests: ida y vuelta completa en modo recursivo (incluida la subcarpeta
      borrada), carpetas no vacías que se conservan, journal que sobrevive a
      una pasada sin cambios, rechazo de `../`, carpeta sin journal y el
      parseo de `--undo`.

## Fase 7: Workspace + GUI

- [ ] Pasar a un workspace:
  ```text
  crates/
  ├── organizer-core/   # src/{lib,plan,executor,event,fs_ops,grouping}.rs
  ├── organizer-cli/    # src/{main.rs, cli/}
  └── organizer-gui/
  ```
  Así las dependencias de la GUI no inflan el binario de la CLI.
- [ ] Añadir al núcleo lo que una GUI necesita y la CLI no:
  - `Event::Progress { done, total }`.
  - Cancelación cooperativa: `cancel: &AtomicBool`, comprobada entre archivos.
  - Ejecutar sobre un subconjunto del plan (el usuario desmarca filas):
    `Plan::retain(...)` o `execute` sobre `&[PlannedMove]`.
- [ ] GUI con **`eframe`/`egui`** (recomendado: Rust puro, un único binario,
      poco código). La alternativa es Tauri si se quiere una UI web.
  - Selector de carpeta, opciones (granularidad, filtros), tabla del `Plan`
    con casillas, botón Ejecutar, barra de progreso, log de eventos y botón
    Deshacer.
  - `execute` en un hilo de trabajo que envía los `Event` por `mpsc` a la UI.

---

## Seguimiento

| Fase | Estado |
|------|--------|
| 1. Higiene | ✅ (salvo `repository`, que espera a que exista un remoto) |
| 2. lib/bin + eventos | ✅ |
| 3. Claridad + bugs | ✅ |
| 4. clap + `--by-day` | ✅ |
| 5. Features sobre `Options` | ✅ (EXIF pendiente; log → fase 6) |
| 6. Journal + `--undo` + log | ✅ |
| 7. Workspace + GUI | ⏳ |
