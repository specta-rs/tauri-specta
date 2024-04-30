//! This file contains a trait that is implemented for all Tauri types supporting the `listen` event.
//! It would be really nice if we could upstream this into Tauri.

use tauri::{Event, EventId, Runtime};

/// An extension trait to `tauri::Manager` to provide access to other methods.
pub trait ManagerExt<R: Runtime>: tauri::Manager<R> {
    fn listen<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: Fn(Event) + Send + 'static;

    fn once<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: FnOnce(Event) + Send + 'static;

    fn unlisten(&self, id: EventId);
}

impl<R: Runtime> ManagerExt<R> for tauri::App<R> {
    fn listen<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: Fn(Event) + Send + 'static,
    {
        self.listen(event, handler)
    }

    fn once<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: FnOnce(Event) + Send + 'static,
    {
        self.once(event, handler)
    }

    fn unlisten(&self, id: EventId) {
        self.unlisten(id)
    }
}

impl<R: Runtime> ManagerExt<R> for tauri::AppHandle<R> {
    fn listen<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: Fn(Event) + Send + 'static,
    {
        self.listen(event, handler)
    }

    fn once<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: FnOnce(Event) + Send + 'static,
    {
        self.once(event, handler)
    }

    fn unlisten(&self, id: EventId) {
        self.unlisten(id)
    }
}

impl<R: Runtime> ManagerExt<R> for tauri::Webview<R> {
    fn listen<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: Fn(Event) + Send + 'static,
    {
        self.listen(event, handler)
    }

    fn once<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: FnOnce(Event) + Send + 'static,
    {
        self.once(event, handler)
    }

    fn unlisten(&self, id: EventId) {
        self.unlisten(id)
    }
}

impl<R: Runtime> ManagerExt<R> for tauri::WebviewWindow<R> {
    fn listen<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: Fn(Event) + Send + 'static,
    {
        self.listen(event, handler)
    }

    fn once<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: FnOnce(Event) + Send + 'static,
    {
        self.once(event, handler)
    }

    fn unlisten(&self, id: EventId) {
        self.unlisten(id)
    }
}

impl<R: Runtime> ManagerExt<R> for tauri::Window<R> {
    fn listen<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: Fn(Event) + Send + 'static,
    {
        self.listen(event, handler)
    }

    fn once<F>(&self, event: impl Into<String>, handler: F) -> EventId
    where
        F: FnOnce(Event) + Send + 'static,
    {
        self.once(event, handler)
    }

    fn unlisten(&self, id: EventId) {
        self.unlisten(id)
    }
}
