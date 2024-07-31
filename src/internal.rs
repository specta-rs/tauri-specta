//! Internal logic for Tauri Specta.
//! Nothing in this module has to conform to semver so it should not be used outside of this crate.
//! It has to be public so macro's can access it.

use std::sync::Arc;

use specta::{datatype, TypeMap};
use tauri::{ipc::Invoke, Runtime};

use crate::{EventCollection, EventDataType};

type SpectaCollectTypes = fn(&mut TypeMap) -> Vec<datatype::Function>;

/// A wrapper around the output of the `collect_commands` macro.
/// This acts to seal the implementation details of the macro.
pub struct Commands<R: Runtime>(
    // Bounds copied from `tauri::Builder::invoke_handler`
    pub(crate) Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    pub(crate) SpectaCollectTypes,
);

impl<R: Runtime> Default for Commands<R> {
    fn default() -> Self {
        Self(
            Arc::new(tauri::generate_handler![]),
            ::specta::function::collect_functions![],
        )
    }
}

/// A wrapper around the output of the `collect_commands` macro.
/// This acts to seal the implementation details of the macro.
#[derive(Default)]
pub struct Events(
    pub(crate) EventCollection,
    pub(crate) Vec<EventDataType>,
    pub(crate) TypeMap,
);

/// called by `collect_commands` to construct `Commands`
pub fn command<R: Runtime, F>(f: F, types: SpectaCollectTypes) -> Commands<R>
where
    F: Fn(Invoke<R>) -> bool + Send + Sync + 'static,
{
    Commands(Arc::new(f), types)
}

/// called by `collect_events` to construct `Events`
pub fn events(a: EventCollection, b: Vec<EventDataType>, c: TypeMap) -> Events {
    Events(a, b, c)
}
