/// Collect events and their types.
///
/// This returns a [`Events`](crate::Events) struct that can be passed to [`Builder::events`](crate::Builder::events).
///
/// # Usage
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use specta::Type;
/// use tauri_specta::{Event, Builder, collect_events};
///
/// #[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
/// pub struct MyEvent(String);
///
/// #[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
/// pub struct MyGenericEvent<T: Type + 'static>(T);
///
/// mod hello {
/// # use serde::{Serialize, Deserialize};
/// # use specta::Type;
/// # use tauri_specta::{Event, Builder, collect_events};
///     use super::*;
///
///     #[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
///     pub struct World(String);
/// }
///
/// let mut builder = Builder::<tauri::Wry>::new()
///     .events(collect_events![
///         // You can pass a struct name.
///         MyEvent,
///         // You can also pass a module.
///         hello::World,
///         // or you can specify generics.
///         MyGenericEvent::<String>
///     ]);
/// ```
///
/// # Invalid event names
///
/// Tauri only allows alphanumeric characters, `-`, `/`, `:` and `_` in an event
/// name. A derived name that cannot be emitted or listened to is rejected at compile
/// time rather than failing during `emit` or `listen`.
///
/// ```compile_fail
/// # use serde::{Serialize, Deserialize};
/// # use specta::Type;
/// # use tauri_specta::Event;
/// #[derive(Serialize, Deserialize, Type, Event)]
/// #[tauri_specta(event_name = "bad\"name")]
/// struct InvalidEvent(String);
/// ```
///
#[macro_export]
macro_rules! collect_events {
    ($($event:path),* $(,)?) => {{
        let mut events: $crate::Events = ::core::default::Default::default();
        $($crate::internal::register_event::<$event>(&mut events);)*
        events
    }};
}
