/// Collect commands and their types.
///
/// This is a combination of Tauri's [`generate_handler`](tauri::generate_handler) and Specta's [`collect_functions`](specta::function::collect_functions),
/// returning a [`Commands`](crate::Commands) struct that can be passed to [`Builder::commands`](crate::Builder::commands).
///
/// It can be used with generic functions as well.
/// # Usage
/// ```
/// use tauri_specta::{collect_commands,Builder};
///
/// #[tauri::command]
/// #[specta::specta] // < You must annotate your commands
/// fn hello_world(my_name: String) -> String {
///     format!("Hello, {my_name}! You've been greeted from Rust!")
/// }
///
/// let mut builder = Builder::<tauri::Wry>::new()
///     .commands(collect_commands![hello_world]);
/// ```
///
#[macro_export]
macro_rules! collect_commands {
    ($($b:ident $(:: $($p:ident)? $(<$($g:path),*>)? )* ),* $(,)?) => {
        // We strip generics (::<...>) from being parsed to Tauri as it doesn't support them.
        $crate::internal::command(
            ::tauri::generate_handler![$($b $($(::$p)? )* ),*],
            ::specta::function::collect_functions![$($b $($(::$p)? $(::<$($g),*>)? )* ),*],
        )
    };
}

/// Collect events and their types.
///
/// This returns a [`Events`](crate::Events) struct that can be passed to [`Builder::events`](crate::Builder::events).
///
/// # Usage
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use specta::Type;
/// use tauri_specta::{Event,Builder,collect_events};
///
/// #[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
/// pub struct MyEvent(String);
///
/// let mut builder = Builder::<tauri::Wry>::new()
///     .events(collect_events![MyEvent]);
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
