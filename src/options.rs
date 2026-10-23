use crate::grouping::Granularity;

/// Cómo escanear y agrupar. Punto de extensión para nuevas features
/// (filtros por extensión, modo recursivo, fuente de la fecha…): se añaden
/// aquí como campos con un valor por defecto que conserva el comportamiento
/// actual.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub granularity: Granularity,
}
