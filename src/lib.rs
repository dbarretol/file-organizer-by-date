//! Núcleo de File Organizer by Date: planificar y ejecutar el movimiento de
//! archivos sueltos a carpetas por fecha de modificación.
//!
//! No imprime ni lee de stdin: devuelve datos ([`Plan`], [`Summary`]) y emite
//! [`Event`]s, para que cualquier frontend (CLI, GUI) decida cómo mostrarlos.

mod event;
mod executor;
mod fs_ops;
mod grouping;
mod options;
mod plan;

pub use event::{Event, Failure};
pub use executor::{execute, Summary};
pub use grouping::{folder_for, Granularity};
pub use options::Options;
pub use plan::{scan, Plan, PlannedMove, ScanWarning};
