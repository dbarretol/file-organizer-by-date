# Diseño de la GUI (trabajo futuro)

> **Estado:** diseño aprobado, sin implementar. Se desarrollará en una rama
> propia (propuesta: `feature/gui`) a partir de `main` una vez integrada
> `refactor/core-lib`.
>
> **Requisito previo:** el núcleo ya está preparado (fases 1 a 6 de
> [`REFACTOR_PLAN.md`](REFACTOR_PLAN.md)). `scan`, `execute`, `undo`,
> `Options`, `Event` y `Journal` no imprimen ni leen stdin, así que la GUI los
> consume directamente.

## Índice

1. [Objetivos y usuario](#1-objetivos-y-usuario)
2. [Principios de usabilidad](#2-principios-de-usabilidad)
3. [Tecnología](#3-tecnología)
4. [Arquitectura](#4-arquitectura)
5. [Cambios necesarios en el núcleo](#5-cambios-necesarios-en-el-núcleo)
6. [Flujo y pantallas](#6-flujo-y-pantallas)
7. [Detalle de componentes](#7-detalle-de-componentes)
8. [Estados, errores y casos límite](#8-estados-errores-y-casos-límite)
9. [Accesibilidad e idioma](#9-accesibilidad-e-idioma)
10. [Persistencia de preferencias](#10-persistencia-de-preferencias)
11. [Empaquetado y distribución](#11-empaquetado-y-distribución)
12. [Estrategia de pruebas](#12-estrategia-de-pruebas)
13. [Plan de trabajo por hitos](#13-plan-de-trabajo-por-hitos)
14. [Criterios de aceptación](#14-criterios-de-aceptación)
15. [Riesgos y decisiones abiertas](#15-riesgos-y-decisiones-abiertas)

---

## 1. Objetivos y usuario

**Para quién:** alguien que no usa la terminal y quiere ordenar una carpeta
caótica (Descargas, fotos del móvil, escritorio) sin miedo a perder archivos.

**Qué debe poder hacer, en orden de importancia:**

1. Elegir una carpeta, ver **qué va a pasar** y confirmarlo, en menos de 30
   segundos y sin leer documentación.
2. **Deshacer** con un clic si el resultado no le gusta.
3. Ajustar opciones comunes (por año, mes o día; solo fotos; incluir
   subcarpetas) sin entender conceptos técnicos.
4. Excluir archivos concretos antes de ejecutar.

**Fuera de alcance de la primera versión:** reglas personalizadas, varias
carpetas a la vez, programar ejecuciones, vigilar carpetas en segundo plano y
EXIF.

---

## 2. Principios de usabilidad

| # | Principio | Cómo se aplica |
|---|-----------|----------------|
| P1 | **Nada se mueve sin una vista previa y un clic explícito** | El botón "Organizar" solo se activa con un plan calculado y visible. No hay modo "sin confirmar" en la GUI |
| P2 | **Siempre se puede volver atrás** | Tras organizar, "Deshacer" ocupa el lugar del botón principal. Al abrir una carpeta con journal se ofrece deshacer la ejecución anterior |
| P3 | **Valores por defecto seguros y útiles** | Mes, fecha de modificación, sin recursión y todos los tipos: lo mismo que la CLI sin flags |
| P4 | **Lenguaje llano** | "Incluir subcarpetas" en lugar de "recursivo"; "Fecha de creación" en lugar de "ctime"; nada de `Y2024-M01` sin un ejemplo al lado |
| P5 | **Feedback inmediato** | Cambiar una opción recalcula la vista previa (con *debounce*). Las operaciones largas muestran progreso y se pueden cancelar |
| P6 | **Errores explicados y accionables** | Cada error dice qué pasó, a qué archivo afecta y qué puede hacer el usuario. No se muestran códigos de error del SO sin contexto |
| P7 | **Una sola ventana y un flujo lineal** | Elegir, revisar, organizar y ver el resultado o deshacer. Sin pestañas ni menús anidados |
| P8 | **La interfaz nunca se congela** | Escanear, mover y deshacer corren en un hilo de trabajo |

---

## 3. Tecnología

**Decisión: `eframe`/`egui`**, la versión estable más reciente en el momento
de crear la rama.

| Criterio | egui (elegido) | Tauri (alternativa) |
|----------|----------------|---------------------|
| Lenguaje | 100 % Rust | Rust + HTML/CSS/JS |
| Binario | Uno, sin runtime | Necesita WebView2 (Windows) o WebKitGTK |
| Integración con el núcleo | Llamada directa | Comandos IPC serializados |
| Aspecto | Funcional, no nativo | Libre (web) |
| Coste de mantenimiento | Bajo | Dos ecosistemas |

**Crates de apoyo:**

| Crate | Uso |
|-------|-----|
| `eframe` (+ feature `persistence`) | Ventana, bucle de eventos y guardado de preferencias |
| `egui_extras` | `TableBuilder` con filas virtualizadas (miles de archivos) |
| `rfd` | Diálogo nativo "Seleccionar carpeta" |
| `opener` | "Abrir carpeta en el explorador" al terminar |
| `egui_kittest` (dev) | Tests de interacción con la UI |

> **MSRV:** `eframe` suele exigir un Rust más reciente que el núcleo (hoy
> 1.82). Por eso la GUI va en su propio crate con su propio `rust-version`:
> el núcleo y la CLI conservan la suya.

---

## 4. Arquitectura

### 4.1 Workspace

Primer paso de la rama: convertir el repo en un workspace **sin cambiar el
comportamiento de la CLI**.

```text
Cargo.toml                    # [workspace] members = ["crates/*"]
crates/
├── organizer-core/           # src/{lib,plan,executor,event,fs_ops,grouping,
│                             #      options,filter,journal}.rs (movidos tal cual)
├── organizer-cli/            # src/{main.rs, cli/}  → binario `file-organizer-by-date`
└── organizer-gui/            # binario `file-organizer-gui`
    └── src/
        ├── main.rs           # arranque de eframe, icono, tamaño mínimo
        ├── app.rs            # impl eframe::App: solo dibuja y despacha mensajes
        ├── state.rs          # AppState + Message + update(): lógica pura, testeable
        ├── worker.rs         # hilo de trabajo: scan / execute / undo → canal
        ├── views/
        │   ├── folder.rs     # selector de carpeta + aviso de "deshacer anterior"
        │   ├── options.rs    # panel de opciones
        │   ├── preview.rs    # tabla agrupada con casillas
        │   ├── progress.rs   # barra + log en vivo + cancelar
        │   └── result.rs     # resumen + deshacer + abrir carpeta
        └── text.rs           # todas las cadenas visibles (base para i18n)
```

- **El nombre del paquete de la CLI se mantiene** (`file-organizer-by-date`),
  para no romper instalaciones ni scripts.
- **CI:** se añade `organizer-gui` a la matriz. En Linux hacen falta las
  dependencias de sistema de eframe; si el crate tiene otra MSRV, también un
  job propio.

### 4.2 Modelo de hilos

```text
┌──────────── hilo UI (egui) ────────────┐        ┌──── hilo de trabajo ────┐
│ AppState ──Message──► update()         │        │                         │
│     ▲                    │ Command      │  cmd   │ scan(dir, &options)     │
│     │                    └─────────────┼───────►│ execute(plan, on_event, │
│     │                                  │        │         &cancel)        │
│     │   WorkerMsg (Plan, Event,        │  msgs  │ undo(base, &journal, …) │
│     └── Summary, UndoResult, error) ◄──┼────────│                         │
│         + ctx.request_repaint()        │        └─────────────────────────┘
└────────────────────────────────────────┘
```

- **Un único hilo de trabajo de larga vida**, que recibe `Command` por
  `std::sync::mpsc`. Basta, porque la app nunca hace dos operaciones de disco
  a la vez.
- **El worker envía cada `Event` por el canal** y llama a
  `egui::Context::request_repaint()` para que la UI despierte.
- **Cancelación:** un `Arc<AtomicBool>` compartido (ver §5.2).
- **Re-escaneos por cambio de opciones:** cada `Command::Scan` lleva un
  `generation: u64`. La UI descarta los `Plan` de generaciones antiguas, sin
  necesidad de bloqueos.

### 4.3 Estado como máquina de estados

```rust
enum Screen {
    PickFolder,                                   // sin carpeta
    Scanning { generation: u64 },
    Review { plan: Plan, selection: Vec<bool> },  // vista previa editable
    Running { total: usize, done: usize, log: Vec<LogLine> },
    Done { summary: Summary, journal_saved: bool },
    Undoing { total: usize, done: usize, log: Vec<LogLine> },
    Undone { summary: Summary },
}

struct AppState {
    folder: Option<PathBuf>,
    options: Options,
    previous_run: Option<Journal>,   // Journal::load al elegir carpeta
    screen: Screen,
    banner: Option<Banner>,          // errores/avisos no bloqueantes
}
```

`update(state, msg) -> Vec<Command>` es una **función pura**: toda la lógica
de flujo se prueba sin abrir una ventana (§12). `app.rs` solo dibuja `state`
y convierte clics en `Message`.

---

## 5. Cambios necesarios en el núcleo

Son pequeños y retrocompatibles. Van en la rama de la GUI, con sus tests, y
la CLI los aprovecha donde tenga sentido.

### 5.1 Progreso: no requiere cambios

**Invariante** que hay que documentar y fijar con un test: cada `PlannedMove`
produce exactamente un evento terminal (`Moved`, `SourceMissing` o `Failed`),
y lo mismo cada `JournalMove` en `undo`. La GUI calcula
`done / plan.moves.len()` contando esos eventos, sin un `Event::Progress`.

### 5.2 Cancelación cooperativa (necesario)

```rust
pub fn execute_cancellable(
    plan: &Plan,
    on_event: &mut dyn FnMut(Event),
    cancel: &AtomicBool,
) -> Summary;            // Summary gana `cancelled: bool`
```

- **Se comprueba `cancel` entre archivos**, nunca a mitad de un movimiento:
  un archivo está o no está movido, nunca a medias.
- **`execute` pasa a delegar** en `execute_cancellable` con un `AtomicBool`
  a `false`, así que la API actual no cambia.
- **El journal registra lo que sí se movió:** "Deshacer" funciona también
  tras una cancelación. Hay que probarlo con un test.
- Lo mismo para `undo` (`undo_cancellable`).

### 5.3 Ejecutar solo lo seleccionado: no requiere cambios

`Plan::moves` es público. La GUI construye un `Plan` filtrado:

```rust
let selected = Plan {
    base: plan.base.clone(),
    moves: plan.moves.iter().zip(&selection).filter(|(_, s)| **s).map(|(m, _)| m.clone()).collect(),
    warnings: Vec::new(),
};
```

Requiere `#[derive(Clone)]` en `PlannedMove`, un cambio trivial.

### 5.4 Datos para la tabla (recomendado)

- **`PlannedMove::date: DateTime<Local>`**: la fecha usada para clasificar,
  para mostrarla en la tabla sin volver a leer metadatos.
- **`PlannedMove::size: u64`**: el tamaño, para el total "1,2 GB en 340
  archivos".
- **Opcional: `Plan::predicted_conflicts()`.** Marca los archivos cuyo
  nombre ya existe en el destino, para mostrar "se renombrará" antes de
  ejecutar. Es solo informativo: el ejecutor sigue resolviendo los conflictos
  reales.

### 5.5 Mensajes de error legibles

Hay que implementar `Display` para `Failure` y `ScanWarning` en el núcleo
(hoy el texto vive en `cli/render.rs`). Así la GUI y la CLI comparten la
redacción base, y la CLI solo añade iconos y colores.

---

## 6. Flujo y pantallas

```text
 ┌─────────────┐  carpeta   ┌──────────┐  plan   ┌──────────┐  Organizar  ┌─────────┐  fin  ┌───────────┐
 │ PickFolder  │──────────► │ Scanning │───────► │  Review  │───────────► │ Running │─────► │   Done    │
 └─────────────┘            └──────────┘         └──────────┘             └─────────┘       └───────────┘
        ▲                         ▲   cambia opción   │  │                     │ Cancelar        │ Deshacer
        │                         └───────────────────┘  │ "Cambiar carpeta"   ▼                 ▼
        └────────────────────────────────────────────────┘                   Done (parcial)   Undoing ─► Undone
```

### 6.1 Ventana principal (pantalla Review)

Ventana mínima de 900×600, redimensionable. El panel de opciones queda
siempre visible a la izquierda; el contenido cambia según la pantalla.

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│  📂 File Organizer by Date                                                   │
├──────────────────────────────────────────────────────────────────────────────┤
│  Carpeta: [ C:\Users\ana\Descargas                        ] [Cambiar…]       │
│  ↩️ Esta carpeta se organizó el 2026-10-20 14:03 (312 archivos). [Deshacer]  │
├────────────────────────┬─────────────────────────────────────────────────────┤
│ OPCIONES               │  Vista previa — 340 archivos · 1,2 GB · 14 carpetas │
│                        │  [🔍 Buscar archivo…      ]  [Todos] [Ninguno]      │
│ Agrupar por            │ ┌─────────────────────────────────────────────────┐ │
│  ○ Año      2024       │ │☑ ▼ 📁 2024 · Enero (Y2024-M01)        23 arch.  │ │
│  ● Mes      2024-M01   │ │   ☑ foto_playa.jpg        15/01/2024   2,1 MB   │ │
│  ○ Día   2024-M01-D15  │ │   ☑ informe.pdf           20/01/2024   340 KB   │ │
│                        │ │   ☐ factura.pdf  ⚠ se renombrará      12 KB    │ │
│ Usar fecha de          │ │☑ ▶ 📁 2024 · Marzo (Y2024-M03)        41 arch.  │ │
│  ● Modificación        │ │◪ ▶ 📁 2025 · Noviembre (Y2025-M11)     8 arch.  │ │
│  ○ Creación            │ │                    …                            │ │
│                        │ └─────────────────────────────────────────────────┘ │
│ Tipos de archivo       │  ⚠ 2 elementos omitidos (enlaces, sin fecha) [Ver] │
│  ● Todos               │                                                     │
│  ○ Solo: [Fotos ▾]     │                                                     │
│    jpg, png, heic …    │                                                     │
│                        │                                                     │
│ ☐ Incluir subcarpetas  │       [ Cancelar ]   [ ✔ Organizar 339 archivos ]   │
└────────────────────────┴─────────────────────────────────────────────────────┘
```

### 6.2 Progreso (Running / Undoing)

```text
│  Organizando… 128 de 339                                                     │
│  [██████████████░░░░░░░░░░░░░░░░░░░░░░░░]  38 %                              │
│  ✔ foto_playa.jpg → 2024 · Enero                                             │
│  ⚠ factura.pdf ya existía → factura_1.pdf                                    │
│  ✔ informe.pdf → 2024 · Enero                                                │
│                                                       [ Cancelar ]           │
```

### 6.3 Resultado (Done)

```text
│  ✅ Listo: 337 archivos organizados en 14 carpetas.                          │
│     1 renombrado para no sobrescribir · 0 omitidos · 2 errores [Ver detalles] │
│                                                                              │
│     [ 📂 Abrir carpeta ]   [ ↩️ Deshacer ]   [ Organizar otra carpeta ]      │
```

Si hubo errores, el resumen va en ámbar y "Ver detalles" despliega la lista.
Cada error muestra el archivo, el motivo en lenguaje llano y una sugerencia:
"Ciérralo si está abierto en otro programa y pulsa Reintentar".

---

## 7. Detalle de componentes

### 7.1 Selección de carpeta

- Hay tres formas de elegirla:
  - el botón "Elegir carpeta…", con el diálogo nativo de `rfd`;
  - **arrastrar y soltar** una carpeta sobre la ventana (`egui` expone
    `dropped_files`);
  - pegar o escribir la ruta, validada al pulsar Enter.
- La pantalla inicial (PickFolder) es una gran zona de soltar con el texto
  "Arrastra aquí una carpeta o pulsa Elegir carpeta".
- **Al elegir carpeta:**
  - `Journal::load` muestra, si existe, el aviso "↩️ Esta carpeta se organizó
    el … [Deshacer]";
  - se lanza `Scan` automáticamente, sin un botón "Analizar".
- **Protección:** antes de escanear la raíz de una unidad (`C:\`, `/`) o la
  carpeta de usuario completa, se pide una confirmación adicional: "Esta
  carpeta contiene muchos archivos del sistema…".

### 7.2 Panel de opciones

Cada control se corresponde exactamente con un campo de `Options`:

| Control | Campo | Detalle de UX |
|---------|-------|---------------|
| Agrupar por: Año / Mes / Día | `granularity` | Cada opción muestra un ejemplo vivo con la fecha de hoy |
| Usar fecha de: Modificación / Creación | `date_source` | Tooltip: "La de creación puede no existir en algunos discos; esos archivos se omitirán" |
| Tipos de archivo: Todos / Solo… | `extensions` | Grupos predefinidos (Fotos, Vídeos, Documentos, Música, Comprimidos) más un campo libre de extensiones separadas por comas. Los grupos son datos en `text.rs`, no lógica |
| Incluir subcarpetas | `recursive` | Texto de ayuda: "Los archivos de las subcarpetas se moverán a las carpetas por fecha de esta carpeta. Las carpetas ya organizadas se ignoran" |

- **Los cambios relanzan el escaneo** con un *debounce* de unos 300 ms.
- **La selección manual del usuario se conserva** al re-escanear siempre que
  el archivo siga en el plan (se indexa por `source`).

### 7.3 Tabla de vista previa

- **Filas virtualizadas** con `egui_extras::TableBuilder`, para que 50 000
  filas no ralenticen la interfaz.
- **Agrupada por carpeta destino**, aprovechando que el `Plan` ya viene
  ordenado por `folder_name`. Los grupos se pueden plegar, son de tres
  estados (☑ ◪ ☐) y muestran el número de archivos.
- **El nombre de carpeta se muestra amigable** ("2024 · Enero") con el
  técnico (`Y2024-M01`) entre paréntesis o en un tooltip.
- **Columnas:** casilla, archivo (ruta relativa a la base), fecha usada,
  tamaño y estado ("⚠ se renombrará" si se implementa §5.4).
- **Búsqueda** por nombre y botones Todos / Ninguno.
- **El botón principal refleja la selección:** "Organizar 339 archivos". Con
  0 seleccionados se desactiva.
- **Los avisos del escaneo (`ScanWarning`) se resumen en una línea** con
  enlace "Ver", que abre un panel con la lista. Nunca bloquean.

### 7.4 Ejecución

- **Organizar:**
  - construye el `Plan` filtrado (§5.3);
  - envía `Command::Execute`, con `JournalRecorder` dentro del worker,
    exactamente como hace la CLI;
  - desactiva el panel de opciones durante la ejecución.
- **El log en vivo** muestra los últimos N eventos con autoscroll; el log
  completo se puede copiar al portapapeles.
- **Cancelar** activa el `AtomicBool`. El botón pasa a "Cancelando…" hasta que
  termina el archivo en curso, y el resultado indica "Cancelado: 128 de 339
  movidos (se pueden deshacer)".

### 7.5 Deshacer

- **Está disponible desde dos sitios:** el resultado recién terminado y el
  aviso de "ejecución anterior" al abrir una carpeta.
- **Antes de deshacer**, un diálogo modal muestra "Se devolverán 337 archivos
  a su ubicación original" y una lista resumida (`journal.moves`).
- **Tras deshacer:**
  - si `pending` no está vacío, se informa "3 archivos no se pudieron
    devolver" y se ofrece "Reintentar";
  - `Journal::persist` se ocupa del archivo, igual que en la CLI.

### 7.6 Ventana y sistema

- Al cerrar con una operación en curso: "Hay una operación en curso.
  ¿Cancelarla y salir?". La cancelación es segura (§5.2).
- Atajos de teclado:
  - `Ctrl+O`: elegir carpeta;
  - `Enter`: acción principal;
  - `Ctrl+Z`: deshacer, solo en la pantalla de resultado y siempre con la
    confirmación modal;
  - `Esc`: cancelar o cerrar el diálogo.

---

## 8. Estados, errores y casos límite

| Situación | Comportamiento |
|-----------|----------------|
| Carpeta sin archivos que organizar | Ilustración y texto: "No hay archivos sueltos aquí". Si `recursive` está desactivado, se sugiere activarlo |
| Error al listar la carpeta (permisos) | Banner rojo con la causa y el botón "Elegir otra carpeta" |
| Carpeta borrada o desmontada entre escaneo y ejecución | Cada archivo produce `SourceMissing`. El resumen lo explica y no cuenta como error |
| Archivo en uso (Windows) | `Failed` con un mensaje accionable y "Reintentar" para los fallidos (un nuevo `Plan` con solo esos) |
| Journal corrupto o con rutas inválidas | El aviso "No se puede deshacer: el registro está dañado" ofrece "Ver archivo". Nunca se borra automáticamente |
| Movimientos sin registrar (rutas no UTF-8) | Aviso persistente en el resultado: "N archivos no podrán deshacerse" |
| Escaneo muy largo | Spinner con contador de archivos vistos y "Cancelar". Requiere que `scan` admita cancelación; si no, se descarta el resultado por `generation` |
| Opciones cambiadas durante un escaneo | Se lanza un escaneo nuevo y el plan antiguo se descarta por `generation` |

---

## 9. Accesibilidad e idioma

- **Lectores de pantalla:** egui integra AccessKit. Todos los controles
  necesitan etiquetas reales: nada de iconos sin texto.
- **El color nunca es el único indicador:** errores, avisos y éxitos llevan
  icono y texto además del color.
- **Contraste suficiente** en los temas claro y oscuro (egui los trae ambos);
  por defecto se sigue el tema del sistema.
- **Escala de interfaz** ajustable (`Ctrl +` / `Ctrl -`) mediante
  `pixels_per_point`.
- **Teclado:** todo el flujo se puede completar sin ratón, con orden de
  tabulación lógico.
- **Idioma:**
  - todas las cadenas viven en `text.rs` desde el primer día;
  - la primera versión sale en español, como la CLI;
  - si más adelante hace falta inglés, se migra `text.rs` a `fluent` o
    `rust-i18n` sin tocar las vistas.

---

## 10. Persistencia de preferencias

Se guardan con `eframe::Storage` (feature `persistence`):

- la última carpeta usada (solo se propone; nunca se escanea al arrancar sin
  acción del usuario);
- las últimas `Options`;
- el tamaño y la posición de la ventana, el tema y la escala.

No se persiste nada sobre los archivos: el journal en cada carpeta sigue
siendo la única fuente de verdad para deshacer, y es compatible con la CLI.

---

## 11. Empaquetado y distribución

- **Windows:**
  - `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`, para
    que no se abra una consola;
  - icono embebido (por ejemplo, con `winres`);
  - firma de código, si se distribuye públicamente.
- **macOS:** un bundle `.app` (por ejemplo, con `cargo-bundle`).
- **Linux:** AppImage o `.deb`.
- **Proceso:** `cargo-dist` puede generar instaladores y releases de GitHub
  para la CLI y la GUI desde el mismo workspace.
- **La CLI y la GUI se publican por separado.** La CLI no debe arrastrar las
  dependencias gráficas.

---

## 12. Estrategia de pruebas

1. **Núcleo:** tests unitarios de los cambios de §5:
   - cancelación entre archivos;
   - journal parcial tras cancelar, y que se puede deshacer;
   - invariante de un evento terminal por movimiento.
2. **Lógica de la app:** tests de `update(state, msg)` sin ventana:
   - transiciones válidas;
   - descarte de planes por `generation`;
   - conservación de la selección al re-escanear;
   - "Organizar" desactivado sin selección.
3. **Interacción:** `egui_kittest` para recorrer el flujo principal (elegir
   carpeta, desmarcar un grupo, organizar, deshacer) sobre un `tempdir`.
4. **Manual, antes de cada release**, en Windows, macOS y Linux:
   - carpeta con 10 000 archivos;
   - archivo abierto en otro programa;
   - carpeta en una unidad de red;
   - cancelar a mitad;
   - cerrar la ventana durante una operación.

---

## 13. Plan de trabajo por hitos

Rama: `feature/gui`, con un PR por hito para mantener las revisiones
pequeñas.

| Hito | Contenido | Resultado verificable |
|------|-----------|----------------------|
| G0 | Workspace `crates/{core,cli,gui}`, CI ampliado | La CLI se comporta igual: el script de comparación de salida da diferencias nulas |
| G1 | Núcleo: cancelación, `Clone` y `date`/`size` en `PlannedMove`, `Display` de errores | Tests nuevos en verde. La CLI usa `Display` sin cambiar su salida |
| G2 | Esqueleto de la GUI: `AppState` + `update` + worker + selección de carpeta + escaneo | Elegir carpeta muestra el número de archivos sin congelar la UI |
| G3 | Panel de opciones y tabla de vista previa con selección | Cambiar opciones re-escanea; desmarcar archivos actualiza el botón |
| G4 | Ejecución con progreso, log en vivo, cancelación y resultado | Organizar y cancelar funcionan, y el journal es compatible con `--undo` de la CLI |
| G5 | Deshacer (resultado + aviso al abrir carpeta), reintentar fallidos | Ida y vuelta completa desde la GUI |
| G6 | Accesibilidad, atajos, persistencia, protecciones (raíz de unidad), pulido de textos | Checklist de §9 y §14 completo |
| G7 | Empaquetado por plataforma y documentación (README con capturas) | Instalables descargables desde una release |

---

## 14. Criterios de aceptación

- [ ] Un usuario nuevo organiza una carpeta y la deshace **sin ayuda** en
      menos de un minuto (prueba con 3 personas).
- [ ] Nada se mueve sin vista previa y clic explícito (P1).
- [ ] La UI no se congela con 50 000 archivos; el escaneo y la ejecución
      muestran progreso.
- [ ] Cancelar deja un estado coherente y deshacible.
- [ ] Un journal creado por la GUI se deshace con la CLI, y viceversa.
- [ ] Todo el flujo se puede completar solo con teclado; los controles tienen
      etiquetas para lectores de pantalla.
- [ ] La CLI no cambia de comportamiento ni de dependencias.
- [ ] CI en verde en Windows, macOS y Linux.

---

## 15. Riesgos y decisiones abiertas

| Tema | Riesgo / pregunta | Propuesta |
|------|-------------------|-----------|
| MSRV de eframe | Más alta que la del núcleo | `rust-version` propio en `organizer-gui` |
| Aspecto no nativo de egui | Puede parecer "poco pulido" | Tema cuidado, iconos coherentes y espaciado generoso. Si con el tiempo pesa, la arquitectura (núcleo sin UI) permite migrar a Tauri |
| Escaneos enormes | `scan` no se puede cancelar hoy | Añadir `cancel` también a `scan` en G1 si las pruebas de G2 lo justifican |
| Journal oculto en Windows | El punto no oculta el archivo | Opcional: atributo `HIDDEN` al crearlo (en Windows hay que manejar bien el reemplazo de un archivo oculto) |
| Idioma | ¿Hace falta inglés en la primera versión? | No; `text.rs` lo deja preparado |
| Integración con el SO | "Organizar con…" en el menú contextual del explorador | Fuera de alcance; posible v2 |
