use std::fs::Metadata;
use std::io;
use std::time::SystemTime;

use crate::filter::ExtensionFilter;
use crate::grouping::Granularity;

/// Cómo escanear y agrupar. Punto de extensión para nuevas features: se
/// añaden aquí como campos con un valor por defecto que conserva el
/// comportamiento actual.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub granularity: Granularity,
    pub date_source: DateSource,
    /// `None` = todos los archivos.
    pub extensions: Option<ExtensionFilter>,
    /// Entra en subcarpetas (sin seguir enlaces y saltando las carpetas
    /// destino `Y####…`). Los archivos encontrados se mueven a las carpetas
    /// destino de la carpeta base.
    pub recursive: bool,
}

/// Qué fecha del archivo decide su carpeta destino.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DateSource {
    #[default]
    Modified,
    /// No disponible en todas las plataformas y sistemas de archivos: si
    /// falla, el archivo se omite con un aviso.
    Created,
}

impl DateSource {
    pub(crate) fn read(self, metadata: &Metadata) -> io::Result<SystemTime> {
        match self {
            DateSource::Modified => metadata.modified(),
            DateSource::Created => metadata.created(),
        }
    }
}
