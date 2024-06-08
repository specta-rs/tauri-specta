use std::{
    collections::{BTreeMap, BTreeSet},
    sync::RwLock,
};

use serde::{de::DeserializeOwned, Serialize};
use specta::{DataType, NamedType, SpectaID};
use tauri::{EventId, EventTarget, Manager, Runtime};

use crate::{ManagerExt, PluginName};

#[derive(Clone, Copy)]
pub struct EventRegistryMeta {
    plugin_name: Option<PluginName>,
}

impl EventRegistryMeta {
    fn wrap_with_plugin(&self, input: &str) -> String {
        self.plugin_name
            .map(|n| n.apply_as_prefix(input, crate::ItemType::Event))
            .unwrap_or_else(|| input.to_string())
    }
}

#[derive(Default)]
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

#[derive(Default)]
pub(crate) struct EventRegistry(pub(crate) RwLock<BTreeMap<SpectaID, EventRegistryMeta>>);

impl EventRegistry {
    pub fn register_collection(
        &self,
        collection: EventCollection,
        plugin_name: Option<PluginName>,
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

    fn listen<F, R: Runtime>(handle: &impl ManagerExt<R>, handler: F) -> EventId
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        let meta = get_meta!(handle);

        handle.listen(meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }

    fn listen_any<F, R: Runtime>(handle: &impl Manager<R>, handler: F) -> EventId
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        let meta = get_meta!(handle);

        handle.listen_any(meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }

    fn once<F, R: Runtime>(handle: &impl ManagerExt<R>, handler: F) -> EventId
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        let meta = get_meta!(handle);

        handle.once(meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }

    fn once_any<F, R: Runtime>(handle: &impl Manager<R>, handler: F) -> EventId
    where
        F: FnOnce(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        let meta = get_meta!(handle);

        handle.once_any(meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }

    fn emit<R: Runtime>(&self, handle: &impl Manager<R>) -> tauri::Result<()>
    where
        Self: Serialize + Clone,
    {
        let meta = get_meta!(handle);

        handle.emit(&meta.wrap_with_plugin(Self::NAME), self)
    }

    fn emit_to<R: Runtime>(&self, handle: &impl Manager<R>, label: &str) -> tauri::Result<()>
    where
        Self: Serialize + Clone,
    {
        let meta = get_meta!(handle);

        handle.emit_to(label, &meta.wrap_with_plugin(Self::NAME), self)
    }

    fn emit_filter<F, R: Runtime>(&self, handle: &impl Manager<R>, filter: F) -> tauri::Result<()>
    where
        F: Fn(&EventTarget) -> bool,
        Self: Serialize + Clone,
    {
        let meta = get_meta!(handle);

        handle.emit_filter(&meta.wrap_with_plugin(Self::NAME), self, filter)
    }
}

pub struct EventDataType {
    pub name: &'static str,
    pub typ: DataType,
}

#[doc(hidden)]
pub type CollectEventsTuple = (EventCollection, Vec<EventDataType>, specta::TypeMap);

#[macro_export]
macro_rules! collect_events {
    ($($event:path),* $(,)?) => {{
    	let mut collection: $crate::EventCollection = ::core::default::Default::default();

     	$(collection.register::<$event>();)*

      	let mut type_map = Default::default();

      	let event_data_types = [$(
	       $crate::EventDataType {
	       		name: <$event as $crate::Event>::NAME,
	       		typ: <$event as ::specta::Type>::reference(&mut type_map, &[]).inner
	       }
       	),*]
        .into_iter()
        .collect::<Vec<_>>();

      	let result: $crate::CollectEventsTuple = (collection, event_data_types, type_map);
        result
    }};
}
