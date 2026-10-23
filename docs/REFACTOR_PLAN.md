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
  - build con la MSRV declarada (1.75)

**Criterio de aceptación:** el CI pasa en las tres plataformas.

> ⚠️ **MSRV real en Windows:** con el `Cargo.lock` actual, `chrono` →
> `iana-time-zone` → `windows-core` 0.62 exige Rust 1.82. En Linux, 1.75 es
> correcto (el job de MSRV corre en Ubuntu). Hay dos opciones: subir
> `rust-version` a 1.82, o fijar versiones antiguas de esas dependencias en el
> lock. Queda pendiente de decidir.

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
pub fn scan(dir: &Path) -> io::Result<Plan>;
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

- [ ] `clap` con `derive`. Mantener `RUTA`, `--yes/-y`, `--help/-h` y los
      códigos de salida (clap usa el 2 para errores de uso, el mismo que hoy).
- [ ] `pub struct Options { granularity: Granularity, … }`, que se pasa a `scan`.
- [ ] `pub enum Granularity { Year, Month, Day }`, con `Month` por defecto.
      `folder_for(dt, granularity)`:
  - `Year`: `Y2024`
  - `Month`: `Y2024-M01` (actual)
  - `Day`: `Y2024-M01-D15`
- [ ] Flag `--by-day` (y quizá `--by-year`). Más adelante, `--group-by <year|month|day>`.
- [ ] Tests de `folder_for` para cada granularidad.

## Fase 5: Más features sobre `Options`

Cada una toca un único punto:

| Feature | Punto de extensión |
|---------|--------------------|
| `--extensions jpg,png` | `Options::filter: Option<ExtFilter>`, aplicado en `scan` |
| `--recursive` | `Options::recursive` en `scan`. Excluir las carpetas destino (`Y####…`) para que sea idempotente |
| Fecha de creación / EXIF | `Options::date_source: DateSource { Modified, Created, … }` |
| Log a archivo | Un consumidor más de `Event`, sin tocar el núcleo |
| `--undo` | Ver la fase 6 |

## Fase 6: Journal de movimientos y `--undo`

- [ ] `execute` registra cada `Moved { from, to }` (ya lo emite como evento).
- [ ] La CLI, o un `JournalWriter` del núcleo, lo guarda en
      `<base>/.file-organizer-journal.json` (añadir `serde` + `serde_json`).
- [ ] `undo(journal) -> Summary`: mueve cada `to → from` reutilizando
      `fs_ops::move_file` (nunca sobrescribe), en orden inverso, y borra las
      carpetas que queden vacías.
- [ ] Excluir el propio journal del escaneo.

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
| 4. clap + `--by-day` | ⏳ |
| 5. Features sobre `Options` | ⏳ |
| 6. Journal + `--undo` | ⏳ |
| 7. Workspace + GUI | ⏳ |
