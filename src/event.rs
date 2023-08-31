use std::collections::BTreeSet;

use serde::{de::DeserializeOwned, Serialize};
use specta::DataType;
use tauri::{EventHandler, Manager, Runtime, Window};

#[derive(Default)]
pub struct EventRegistry(pub(crate) BTreeSet<&'static str>);

impl EventRegistry {
    pub fn register<E: Event>(&mut self) {
        self.0.insert(E::NAME);
    }
}

pub struct EventObj<T: Event> {
    pub id: EventHandler,
    pub payload: T,
}

pub trait Event: Serialize + DeserializeOwned + Clone {
    const NAME: &'static str;

    #[cfg(debug_assertions)]
    fn check_event_in_registry_state<R: Runtime>(handle: &impl Manager<R>) {
        let Some(registry) = handle.try_state::<EventRegistry>() else {
       		println!("EventRegistry not found in Tauri state - Did you forget to call Exporter::with_events?");
         	return;
        };

        let name = Self::NAME;

        if !registry.0.contains(name) {
            println!("Event {name} not registered!");
        }
    }

    // Manager functions

    fn emit_all<R: Runtime>(self, handle: &impl Manager<R>) -> tauri::Result<()> {
        #[cfg(debug_assertions)]
        Self::check_event_in_registry_state(handle);

        handle.emit_all(Self::NAME, self)
    }

    fn emit_to<R: Runtime>(self, handle: &impl Manager<R>, label: &str) -> tauri::Result<()> {
        #[cfg(debug_assertions)]
        Self::check_event_in_registry_state(handle);

        handle.emit_to(Self::NAME, label, self)
    }

    fn trigger_global<R: Runtime>(self, handle: &impl Manager<R>) {
        #[cfg(debug_assertions)]
        Self::check_event_in_registry_state(handle);

        handle.trigger_global(Self::NAME, serde_json::to_string(&self).ok());
    }

    fn listen_global<F, R: Runtime>(handle: &impl Manager<R>, handler: F) -> EventHandler
    where
        F: Fn(EventObj<Self>) + Send + 'static,
    {
        #[cfg(debug_assertions)]
        Self::check_event_in_registry_state(&handle.app_handle());

        handle.listen_global(Self::NAME, move |event| {
            let value: serde_json::Value = event
                .payload()
                .and_then(|p| serde_json::from_str(p).ok())
                .unwrap_or(serde_json::Value::Null);

            handler(EventObj {
                id: event.id(),
                payload: serde_json::from_value(value).unwrap(),
            });
        })
    }

    fn once_global<F, R: Runtime>(handle: &impl Manager<R>, handler: F) -> EventHandler
    where
        F: FnOnce(EventObj<Self>) + Send + 'static,
    {
        #[cfg(debug_assertions)]
        Self::check_event_in_registry_state(handle);

        handle.once_global(Self::NAME, move |event| {
            let value: serde_json::Value = event
                .payload()
                .and_then(|p| serde_json::from_str(p).ok())
                .unwrap_or(serde_json::Value::Null);

            handler(EventObj {
                id: event.id(),
                payload: serde_json::from_value(value).unwrap(),
            });
        })
    }

    // Window functions

    fn emit(self, window: &Window<impl Runtime>) -> tauri::Result<()> {
        #[cfg(debug_assertions)]
        Self::check_event_in_registry_state(window);

        window.emit(Self::NAME, self)
    }

    fn trigger(self, window: &Window<impl Runtime>) {
        #[cfg(debug_assertions)]
        Self::check_event_in_registry_state(window);

        window.trigger(Self::NAME, serde_json::to_string(&self).ok());
    }

    fn emit_and_trigger(self, window: &Window<impl Runtime>) -> tauri::Result<()> {
        #[cfg(debug_assertions)]
        Self::check_event_in_registry_state(window);

        window.emit_and_trigger(Self::NAME, self)
    }

    fn listen<F>(window: &Window<impl Runtime>, handler: F) -> EventHandler
    where
        F: Fn(EventObj<Self>) + Send + 'static,
    {
        #[cfg(debug_assertions)]
        Self::check_event_in_registry_state(window);

        window.listen(Self::NAME, move |event| {
            let value: serde_json::Value = event
                .payload()
                .and_then(|p| serde_json::from_str(p).ok())
                .unwrap_or(serde_json::Value::Null);

            handler(EventObj {
                id: event.id(),
                payload: serde_json::from_value(value).unwrap(),
            });
        })
    }

    fn once<F>(window: &Window<impl Runtime>, handler: F) -> EventHandler
    where
        F: FnOnce(EventObj<Self>) + Send + 'static,
    {
        #[cfg(debug_assertions)]
        Self::check_event_in_registry_state(window);

        window.once(Self::NAME, move |event| {
            let value: serde_json::Value = event
                .payload()
                .and_then(|p| serde_json::from_str(p).ok())
                .unwrap_or(serde_json::Value::Null);

            handler(EventObj {
                id: event.id(),
                payload: serde_json::from_value(value).unwrap(),
            });
        })
    }
}

pub struct EventMeta {
    pub name: &'static str,
    pub typ: DataType,
}

pub(crate) type CollectEventsTuple = (
    EventRegistry,
    Result<Vec<EventMeta>, specta::ExportError>,
    specta::TypeMap,
);

#[macro_export]
macro_rules! collect_events {
    ($($event:ident),+) => {{
    	let mut registry: $crate::EventRegistry = ::core::default::Default::default();

     	$(registry.register::<$event>();)+

      	let mut type_map = Default::default();

      	let event_metas = [$(
       		<$event as ::specta::Type>::reference(
       			::specta::DefOpts {
       				type_map: &mut type_map,
       				parent_inline: false
          		},
            	&[]
       		).map(|typ| $crate::EventMeta {
         		name: <$event as $crate::Event>::NAME,
         		typ
         	})
       	),+]
        .into_iter()
        .collect::<Result<Vec<_>, _>>();

      	(registry, event_metas, type_map)
    }};
}
