use std::{borrow::Cow, collections::BTreeSet};

use serde::{de::DeserializeOwned, Serialize};
use specta::{NamedType, SpectaID};
use tauri::{Emitter, EventId, EventTarget, Listener, Manager, Runtime};

use crate::{apply_as_prefix, ItemType};

/// A struct for managing events that is put into Tauri's state.
pub(crate) struct EventRegistry {
    pub(crate) plugin_name: Option<&'static str>,
    pub(crate) events: BTreeSet<SpectaID>,
}

impl EventRegistry {
    /// gets the name of the event (taking into account plugin prefixes) and ensuring it was correctly mounted to the current app.
    pub fn get_event_name<E: Event, R: Runtime>(
        handle: &impl Manager<R>,
        name: &'static str,
    ) -> Cow<'static, str> {
        let this = handle.try_state::<EventRegistry>().expect(
            "EventRegistry not found in Tauri state - Did you forget to call Builder::mount_events?",
        ).inner();

        this.events
            .get(&E::sid())
            .unwrap_or_else(|| panic!("Event {name} not found in registry!"));

        this.plugin_name
            .map(|n| apply_as_prefix(n, name, ItemType::Event).into())
            .unwrap_or_else(|| name.into())
    }
}

/// A typed event that was emitted.
pub struct TypedEvent<T: Event> {
    /// The [`EventId`] of the handler that was triggered.
    pub id: EventId,
    /// The event payload.
    pub payload: T,
}

macro_rules! make_handler {
    ($handler:ident) => {
        move |event| {
            $handler(TypedEvent {
                id: event.id(),
                payload: serde_json::from_str(event.payload())
                    .expect("Failed to deserialize event payload"),
            });
        }
    };
}

/// Extends your event type with typesafe methods for listening to and emitting events.
///
/// You should rely on the [`Event`](macro@crate::Event) derive macro to implement this for you.
///
/// Be aware most methods take anything that implements [`Manager`](tauri::Manager) so you can can scope the message using all of the following Tauri types:
///  - [`App`](https://docs.rs/tauri/2.0.0-beta.16/tauri/struct.App.html)
///  - [`AppHandle`](https://docs.rs/tauri/2.0.0-beta.16/tauri/struct.AppHandle.html)
///  - [`Webview`](https://docs.rs/tauri/2.0.0-beta.16/tauri/webview/struct.Webview.html)
///  - [`WebviewWindow`](https://docs.rs/tauri/2.0.0-beta.16/tauri/webview/struct.WebviewWindow.html)
///  - [`Window`](https://docs.rs/tauri/2.0.0-beta.16/tauri/window/struct.Window.html)
///
///
/// # Example
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use specta::Type;
/// use tauri_specta::Event;
/// use tauri::AppHandle
///
/// #[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
/// pub struct MyEvent(String);
///
/// fn use_event(app: AppHandle) {
///     DemoEvent::listen(handle, |event| {
///         dbg!(event.payload);
///     });
///  
///     DemoEvent("Test".to_string()).emit(handle).ok();
/// }
/// ```
pub trait Event: NamedType {
    /// The unique name for this event. Derived from the struct's name via the [`Event`](macro@crate::Event) derive macro.
    const NAME: &'static str;

    /// Listen to an emitted event on this manager.
    fn listen<F, R: Runtime, H: Listener<R> + Manager<R>>(handle: &H, handler: F) -> EventId
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        handle.listen(
            EventRegistry::get_event_name::<Self, _>(handle, Self::NAME),
            make_handler!(handler),
        )
    }

    /// Listen to an emitted event to any [target](EventTarget).
    fn listen_any<F, R: Runtime, H: Listener<R> + Manager<R>>(handle: &H, handler: F) -> EventId
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        handle.listen_any(
            EventRegistry::get_event_name::<Self, _>(handle, Self::NAME),
            make_handler!(handler),
        )
    }

    /// Listen to an event on this manager only once.
    fn once<F, R: Runtime, H: Listener<R> + Manager<R>>(handle: &H, handler: F) -> EventId
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        handle.once(
            EventRegistry::get_event_name::<Self, _>(handle, Self::NAME),
            make_handler!(handler),
        )
    }

    /// Listens once to an emitted event to any [target](EventTarget) .
    ///
    /// See [`Self::listen_any`] for more information.
    fn once_any<F, R: Runtime, H: Listener<R> + Manager<R>>(handle: &H, handler: F) -> EventId
    where
        F: FnOnce(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        handle.once_any(
            EventRegistry::get_event_name::<Self, _>(handle, Self::NAME),
            make_handler!(handler),
        )
    }

    /// Emits an event to all [targets](EventTarget) matching the given target.
    fn emit<R: Runtime, H: Emitter<R> + Manager<R>>(&self, handle: &H) -> tauri::Result<()>
    where
        Self: Serialize + Clone,
    {
        handle.emit(
            &EventRegistry::get_event_name::<Self, _>(handle, Self::NAME),
            self,
        )
    }

    /// Emits an event to all [targets](EventTarget) matching the given target.
    fn emit_to<R: Runtime, H: Emitter<R> + Manager<R>>(
        &self,
        handle: &H,
        label: &str,
    ) -> tauri::Result<()>
    where
        Self: Serialize + Clone,
    {
        handle.emit_to(
            label,
            &EventRegistry::get_event_name::<Self, _>(handle, Self::NAME),
            self,
        )
    }

    /// Emits an event to all [targets](EventTarget) based on the given filter.
    fn emit_filter<F, R: Runtime, H: Emitter<R> + Manager<R>>(
        &self,
        handle: &H,
        filter: F,
    ) -> tauri::Result<()>
    where
        F: Fn(&EventTarget) -> bool,
        Self: Serialize + Clone,
    {
        handle.emit_filter(
            &EventRegistry::get_event_name::<Self, _>(handle, Self::NAME),
            self,
            filter,
        )
    }
}
