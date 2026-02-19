use std::{fmt, sync::Arc};

use specta::{TypeCollection, datatype};
use tauri::{Runtime, ipc::Invoke};

/// A wrapper around the output of the `collect_commands` macro.
///
/// This acts to seal the implementation details of the macro.
pub struct Commands<R: Runtime>(
    // Bounds copied from `tauri::Builder::invoke_handler`
    pub(crate) Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    pub(crate) fn(&mut TypeCollection) -> Vec<datatype::Function>,
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
            ::specta::function::collect_functions![],
        )
    }
}

impl<R: Runtime> Clone for Commands<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}
