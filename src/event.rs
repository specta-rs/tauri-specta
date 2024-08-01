use std::{
    collections::{BTreeMap, BTreeSet},
    sync::RwLock,
};

use serde::{de::DeserializeOwned, Serialize};
use specta::{DataType, NamedType, SpectaID};
use tauri::{Emitter, EventId, EventTarget, Listener, Manager, Runtime};

use crate::apply_as_prefix;

#[derive(Clone, Copy)]
pub struct EventRegistryMeta {
    plugin_name: Option<&'static str>,
}

impl EventRegistryMeta {
    fn wrap_with_plugin(&self, input: &str) -> String {
        self.plugin_name
            .as_ref()
            .map(|n| apply_as_prefix(n, input, crate::ItemType::Event))
            .unwrap_or_else(|| input.to_string())
    }
}

#[derive(Default, Clone)]
pub struct EventCollection(pub(crate) BTreeSet<SpectaID>, BTreeSet<&'static str>);

impl EventCollection {
    pub fn register<E: Event>(&mut self) {
        if !self.0.insert(E::sid()) {
            panic!("Event {} registered twice!", E::NAME)
        }

        if !self.1.insert(E::NAME) {
            panic!("Another event with name {} is already registered!", E::NAME)
        }
    }
}

// TODO: Should this be pub
#[derive(Default)]
pub(crate) struct EventRegistry(pub(crate) RwLock<BTreeMap<SpectaID, EventRegistryMeta>>);

impl EventRegistry {
    pub fn register_collection(
        &self,
        collection: EventCollection,
        plugin_name: Option<&'static str>,
    ) {
        let mut registry = self.0.write().expect("Failed to write EventRegistry");

        registry.extend(
            collection
                .0
                .into_iter()
                .map(|sid| (sid, EventRegistryMeta { plugin_name })),
        );
    }

    pub fn get_or_manage<R: Runtime>(handle: &impl Manager<R>) -> tauri::State<'_, Self> {
        if handle.try_state::<Self>().is_none() {
            handle.manage(Self::default());
        }

        handle.state::<Self>()
    }
}

pub struct TypedEvent<T: Event> {
    pub id: EventId,
    pub payload: T,
}

fn get_meta_from_registry<R: Runtime>(
    sid: SpectaID,
    name: &str,
    handle: &impl Manager<R>,
) -> EventRegistryMeta {
    handle.try_state::<EventRegistry>().expect(
        "EventRegistry not found in Tauri state - Did you forget to call Exporter::with_events?",
    )
    .0
        .read()
        .expect("Failed to read EventRegistry")
        .get(&sid)
        .copied()
        .unwrap_or_else(|| panic!("Event {name} not found in registry!"))
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

macro_rules! get_meta {
    ($handle:ident) => {
        get_meta_from_registry(Self::sid(), Self::NAME, $handle)
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
            get_meta!(handle).wrap_with_plugin(Self::NAME),
            make_handler!(handler),
        )
    }

    fn listen_any<F, R: Runtime, H: Listener<R> + Manager<R>>(handle: &H, handler: F) -> EventId
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        handle.listen_any(
            get_meta!(handle).wrap_with_plugin(Self::NAME),
            make_handler!(handler),
        )
    }

    fn once<F, R: Runtime, H: Listener<R> + Manager<R>>(handle: &H, handler: F) -> EventId
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        handle.once(
            get_meta!(handle).wrap_with_plugin(Self::NAME),
            make_handler!(handler),
        )
    }

    fn once_any<F, R: Runtime, H: Listener<R> + Manager<R>>(handle: &H, handler: F) -> EventId
    where
        F: FnOnce(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        handle.once_any(
            get_meta!(handle).wrap_with_plugin(Self::NAME),
            make_handler!(handler),
        )
    }

    fn emit<R: Runtime, H: Emitter<R> + Manager<R>>(&self, handle: &H) -> tauri::Result<()>
    where
        Self: Serialize + Clone,
    {
        handle.emit(&get_meta!(handle).wrap_with_plugin(Self::NAME), self)
    }

    fn emit_to<R: Runtime, H: Emitter<R> + Manager<R>>(
        &self,
        handle: &H,
        label: &str,
    ) -> tauri::Result<()>
    where
        Self: Serialize + Clone,
    {
        handle.emit_to(label, &get_meta!(handle).wrap_with_plugin(Self::NAME), self)
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
            &get_meta!(handle).wrap_with_plugin(Self::NAME),
            self,
            filter,
        )
    }
}

#[derive(Debug, Clone)]
pub struct EventDataType {
    pub name: &'static str,
    pub typ: DataType,
}

#[doc(hidden)]
pub type CollectEventsTuple = (EventCollection, Vec<EventDataType>, specta::TypeMap);
