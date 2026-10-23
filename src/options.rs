use std::fs::Metadata;
use std::io;
use std::time::SystemTime;

use crate::filter::ExtensionFilter;
use crate::grouping::Granularity;

/// How to scan and group. Extension point for new features: add them here
/// as fields whose default keeps the current behavior.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub granularity: Granularity,
    pub date_source: DateSource,
    /// `None` = every file.
    pub extensions: Option<ExtensionFilter>,
    /// Descend into subfolders (without following links and skipping the
    /// `Y####…` destination folders). Files found are moved into the
    /// destination folders of the base folder.
    pub recursive: bool,
}

/// Which file date decides its destination folder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DateSource {
    #[default]
    Modified,
    /// Not available on every platform and filesystem: if it fails, the file
    /// is skipped with a warning.
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
