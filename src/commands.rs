use std::{fmt, sync::Arc};

use specta::{Types, datatype};
use tauri::{Runtime, ipc::Invoke};

/// Type-erased collector used by [`Commands`] to register command metadata.
#[doc(hidden)]
pub type CommandTypeCollector =
    dyn Fn(&mut Types) -> Vec<datatype::Function> + Send + Sync + 'static;

/// A wrapper around the output of the `collect_commands` macro.
///
/// This acts to seal the implementation details of the macro.
pub struct Commands<R: Runtime>(
    // TODO: Explain these being public
    // Bounds copied from `tauri::Builder::invoke_handler`
    pub Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    pub Arc<CommandTypeCollector>,
);

impl<R: Runtime> fmt::Debug for Commands<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Commands").finish()
    }
}

impl<R: Runtime> Default for Commands<R> {
    fn default() -> Self {
        Self(
            Arc::new(tauri::generate_handler![]),
            Arc::new(::specta::function::collect_functions![]),
        )
    }
}

impl<R: Runtime> Clone for Commands<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}
