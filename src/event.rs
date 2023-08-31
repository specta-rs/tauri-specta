use std::collections::BTreeSet;

use serde::{de::DeserializeOwned, Serialize};
use specta::DataType;
use tauri::{AppHandle, EventHandler, Manager, Runtime};

pub struct EventRegistry(pub(crate) BTreeSet<&'static str>);

impl EventRegistry {
    pub fn register<E: Event>(&mut self) {
        self.0.insert(E::NAME);
    }
}

impl Default for EventRegistry {
    fn default() -> Self {
        Self(BTreeSet::new())
    }
}

pub struct EventObj<T: Event> {
    pub id: EventHandler,
    pub payload: T,
}

#[cfg(debug_assertions)]
fn check_event_in_registry_state<R: tauri::Runtime>(event: &str, handle: &impl tauri::Manager<R>) {
    let Some(registry) = handle.try_state::<EventRegistry>() else {
    	println!("EventRegistry not found in Tauri state - Did you forget to call Exporter::with_events?");
     	return;
    };

    if !registry.0.contains(event) {
        println!("Event {event} not registered!");
    }
}

pub trait Event: Serialize + DeserializeOwned + Clone {
    const NAME: &'static str;

    fn emit_all<R: Runtime>(self, handle: impl Manager<R>) -> tauri::Result<()> {
        #[cfg(debug_assertions)]
        check_event_in_registry_state(Self::NAME, &handle);

        handle.emit_all(Self::NAME, self)
    }

    fn emit_to<R: Runtime>(self, handle: impl Manager<R>, label: &str) -> tauri::Result<()> {
        #[cfg(debug_assertions)]
        check_event_in_registry_state(Self::NAME, &handle);

        handle.emit_to(Self::NAME, label, self)
    }

    fn trigger_global<R: Runtime>(self, handle: impl Manager<R>) {
        #[cfg(debug_assertions)]
        check_event_in_registry_state(Self::NAME, &handle);

        handle.trigger_global(Self::NAME, serde_json::to_string(&self).ok());
    }

    fn listen_global<F>(handle: AppHandle<impl Runtime>, handler: F)
    where
        F: Fn(EventObj<Self>) + Send + 'static,
    {
        #[cfg(debug_assertions)]
        check_event_in_registry_state(Self::NAME, &handle);

        handle.listen_global(Self::NAME, move |event| {
            let value: serde_json::Value = event
                .payload()
                .and_then(|p| serde_json::from_str(p).ok())
                .unwrap_or(serde_json::Value::Null);

            handler(EventObj {
                id: event.id(),
                payload: serde_json::from_value(value).unwrap(),
            });
        });
    }

    fn once_global<F>(handle: AppHandle<impl Runtime>, handler: F)
    where
        F: FnOnce(EventObj<Self>) + Send + 'static,
    {
        #[cfg(debug_assertions)]
        check_event_in_registry_state(Self::NAME, &handle);

        handle.once_global(Self::NAME, move |event| {
            let value: serde_json::Value = event
                .payload()
                .and_then(|p| serde_json::from_str(p).ok())
                .unwrap_or(serde_json::Value::Null);

            handler(EventObj {
                id: event.id(),
                payload: serde_json::from_value(value).unwrap(),
            });
        });
    }
}

pub struct EventMeta {
    pub name: &'static str,
    pub typ: DataType,
}

pub(crate) type CollectEventsTuple = (
    EventRegistry,
    Result<Vec<EventMeta>, specta::ExportError>,
    specta::TypeDefs,
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
