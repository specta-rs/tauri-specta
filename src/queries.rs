use std::{fmt, sync::Arc};

use specta::{Types, datatype};
use tauri::{Runtime, ipc::Invoke};

/// A wrapper around the output of the `collect_queries` macro.
///
/// Same shape as [`Commands`](crate::Commands) — carries both the Tauri invoke handler
/// and type metadata for TanStack Query `queryOptions` generation.
pub struct Queries<R: Runtime>(
    pub(crate) Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    pub(crate) fn(&mut Types) -> Vec<datatype::Function>,
);

impl<R: Runtime> fmt::Debug for Queries<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Queries").finish()
    }
}

impl<R: Runtime> Default for Queries<R> {
    fn default() -> Self {
        Self(
            Arc::new(tauri::generate_handler![]),
            ::specta::function::collect_functions![],
        )
    }
}

impl<R: Runtime> Clone for Queries<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
    }
}

/// A wrapper around the output of the `collect_mutations` macro.
///
/// Same shape as [`Commands`](crate::Commands) — carries both the Tauri invoke handler
/// and type metadata for TanStack Query `mutationOptions` generation.
pub struct Mutations<R: Runtime>(
    pub(crate) Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    pub(crate) fn(&mut Types) -> Vec<datatype::Function>,
);

impl<R: Runtime> fmt::Debug for Mutations<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mutations").finish()
    }
}

impl<R: Runtime> Default for Mutations<R> {
    fn default() -> Self {
        Self(
            Arc::new(tauri::generate_handler![]),
            ::specta::function::collect_functions![],
        )
    }
}

impl<R: Runtime> Clone for Mutations<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
    }
}

/// Which TanStack Query framework to target for imports.
#[derive(Debug, Clone, Copy)]
pub enum TanstackFramework {
    /// `@tanstack/react-query`
    React,
    /// `@tanstack/solid-query`
    Solid,
    /// `@tanstack/vue-query`
    Vue,
    /// `@tanstack/svelte-query`
    Svelte,
}

impl TanstackFramework {
    /// Returns the npm package name for this framework.
    pub fn package_name(&self) -> &'static str {
        match self {
            Self::React => "@tanstack/react-query",
            Self::Solid => "@tanstack/solid-query",
            Self::Vue => "@tanstack/vue-query",
            Self::Svelte => "@tanstack/svelte-query",
        }
    }
}
