use std::path::Path;

/// Conjunto de extensiones permitidas, sin distinguir mayúsculas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionFilter {
    /// Normalizadas: minúsculas y sin el punto inicial.
    extensions: Vec<String>,
}

impl ExtensionFilter {
    /// Acepta `jpg`, `.jpg` o `JPG` indistintamente; ignora entradas vacías.
    pub fn new<I, S>(extensions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut extensions: Vec<String> = extensions
            .into_iter()
            .map(|e| e.as_ref().trim().trim_start_matches('.').to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
        extensions.sort();
        extensions.dedup();
        ExtensionFilter { extensions }
    }

    pub fn extensions(&self) -> &[String] {
        &self.extensions
    }

    /// Un archivo sin extensión, o con una que no es UTF-8, nunca coincide.
    pub fn matches(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| self.extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_dots_case_and_blanks() {
        let filter = ExtensionFilter::new([".JPG", "png", " ", "jpg"]);
        assert_eq!(filter.extensions(), ["jpg", "png"]);
    }

    #[test]
    fn matches_case_insensitively_on_last_extension() {
        let filter = ExtensionFilter::new(["jpg", "gz"]);
        assert!(filter.matches(Path::new("foto.JPG")));
        assert!(filter.matches(Path::new("backup.tar.gz")));
        assert!(!filter.matches(Path::new("notas.txt")));
        assert!(!filter.matches(Path::new("README")));
        assert!(!filter.matches(Path::new(".jpg")));
    }
}
