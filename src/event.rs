use std::{
    collections::{BTreeMap, BTreeSet},
    sync::RwLock,
};

use serde::{de::DeserializeOwned, Serialize};
use specta::{DataType, NamedType, SpectaID, Type};
use tauri::{EventHandler, Manager, Runtime, Window};

use crate::PluginName;

#[derive(Clone, Copy)]
pub struct EventRegistryMeta {
    plugin_name: PluginName,
}

impl EventRegistryMeta {
    fn wrap_with_plugin(&self, input: &str) -> String {
        self.plugin_name
            .apply_as_prefix(input, crate::ItemType::Event)
    }
}

#[derive(Default)]
pub struct EventCollection(pub(crate) BTreeSet<SpectaID>, BTreeSet<&'static str>);

impl EventCollection {
    pub fn register<E: Event>(&mut self) {
        if !self.0.insert(E::SID) {
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
    pub fn register_collection(&self, collection: EventCollection, plugin_name: PluginName) {
        let mut registry = self.0.write().expect("Failed to write EventRegistry");

        registry.extend(collection.0.into_iter().map(|sid| {
            (
                sid,
                EventRegistryMeta {
                    plugin_name: PluginName::from(plugin_name),
                },
            )
        }));
    }

    pub fn get_or_manage<'a, R: Runtime>(handle: &'a impl Manager<R>) -> tauri::State<'a, Self> {
        if handle.try_state::<Self>().is_none() {
            handle.manage(Self::default());
        }

        handle.state::<Self>()
    }
}

pub struct TypedEvent<T: Event> {
    pub id: EventHandler,
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
            let value: serde_json::Value = event
                .payload()
                .and_then(|p| serde_json::from_str(p).ok())
                .unwrap_or(serde_json::Value::Null);

            $handler(TypedEvent {
                id: event.id(),
                payload: serde_json::from_value(value)
                    .expect("Failed to deserialize event payload"),
            });
        }
    };
}

macro_rules! get_meta {
    ($handle:ident) => {
        get_meta_from_registry(Self::SID, Self::NAME, $handle)
    };
}

pub trait Event: Serialize + DeserializeOwned + Clone + Type + NamedType {
    const NAME: &'static str;

    // Manager functions

    fn emit_all<R: Runtime>(self, handle: &impl Manager<R>) -> tauri::Result<()> {
        let meta = get_meta!(handle);

        handle.emit_all(&meta.wrap_with_plugin(Self::NAME), self)
    }

    fn emit_to<R: Runtime>(self, handle: &impl Manager<R>, label: &str) -> tauri::Result<()> {
        let meta = get_meta!(handle);

        handle.emit_to(&meta.wrap_with_plugin(Self::NAME), label, self)
    }

    fn trigger_global<R: Runtime>(self, handle: &impl Manager<R>) {
        let meta = get_meta!(handle);

        handle.trigger_global(
            &meta.wrap_with_plugin(Self::NAME),
            serde_json::to_string(&self).ok(),
        );
    }

    fn listen_global<F, R: Runtime>(handle: &impl Manager<R>, handler: F) -> EventHandler
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
    {
        let meta = get_meta!(handle);

        handle.listen_global(&meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }

    fn once_global<F, R: Runtime>(handle: &impl Manager<R>, handler: F) -> EventHandler
    where
        F: FnOnce(TypedEvent<Self>) + Send + 'static,
    {
        let meta = get_meta!(handle);

        handle.once_global(&meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }

    // Window functions

    fn emit(self, window: &Window<impl Runtime>) -> tauri::Result<()> {
        let meta = get_meta!(window);

        window.emit(&meta.wrap_with_plugin(Self::NAME), self)
    }

    fn trigger(self, window: &Window<impl Runtime>) {
        let meta = get_meta!(window);

        window.trigger(
            &meta.wrap_with_plugin(Self::NAME),
            serde_json::to_string(&self).ok(),
        );
    }

    fn emit_and_trigger(self, window: &Window<impl Runtime>) -> tauri::Result<()> {
        let meta = get_meta!(window);

        window.emit_and_trigger(&meta.wrap_with_plugin(Self::NAME), self)
    }

    fn listen<F>(window: &Window<impl Runtime>, handler: F) -> EventHandler
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
    {
        let meta = get_meta!(window);

        window.listen(&meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }

    fn once<F>(window: &Window<impl Runtime>, handler: F) -> EventHandler
    where
        F: FnOnce(TypedEvent<Self>) + Send + 'static,
    {
        let meta = get_meta!(window);

        window.once(&meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }
}

pub struct EventDataType {
    pub name: &'static str,
    pub typ: DataType,
}

pub(crate) type CollectEventsTuple = (
    EventCollection,
    Result<Vec<EventDataType>, specta::ExportError>,
    specta::TypeMap,
);

#[macro_export]
macro_rules! collect_events {
    ($($event:ident),+) => {{
    	let mut collection: $crate::EventCollection = ::core::default::Default::default();

     	$(collection.register::<$event>();)+

      	let mut type_map = Default::default();

      	let event_data_types = [$(
       		<$event as ::specta::Type>::reference(
       			::specta::DefOpts {
       				type_map: &mut type_map,
       				parent_inline: false
          		},
            	&[]
       		).map(|typ| $crate::EventDataType {
         		name: <$event as $crate::Event>::NAME,
         		typ
         	})
       	),+]
        .into_iter()
        .collect::<Result<Vec<_>, _>>();

      	(collection, event_data_types, type_map)
    }};
}
