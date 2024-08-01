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

pub struct TypedEvent<T: Event> {
    pub id: EventId,
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

pub trait Event: NamedType {
    const NAME: &'static str;

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

    fn emit<R: Runtime, H: Emitter<R> + Manager<R>>(&self, handle: &H) -> tauri::Result<()>
    where
        Self: Serialize + Clone,
    {
        handle.emit(
            &EventRegistry::get_event_name::<Self, _>(handle, Self::NAME),
            self,
        )
    }

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
