//! TODO

use tauri::Runtime;
use tauri_specta::Commands;

pub struct CommandSet<R: Runtime> {
    pub queries: Commands<R>,
    pub mutations: Commands<R>,
    // TODO: Define events
}

impl<R: Runtime> CommandSet<R> {
    pub fn new(queries: Commands<R>, mutations: Commands<R>) -> Self {
        Self { queries, mutations }
    }

    pub fn merge(&self, other: &Self) -> Self {
        todo!();
    }
}
