use std::{error, io, path::Path};

use crate::BuilderConfiguration;

/// Implemented for all languages which Tauri Specta supports exporting to.
///
/// Currently implemented for:
///  - [`specta_typescript::Typescript`]
///  - [`specta_typescript::JSDoc`]
pub trait LanguageExt {
    /// The error type returned by the language's export function.
    type Error: error::Error + From<io::Error>;

    /// Export to a given path with the language exporter configuration.
    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error>;
}

#[cfg(any(feature = "javascript", feature = "typescript"))]
mod js_ts;
