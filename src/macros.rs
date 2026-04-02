/// Collect commands and their types.
///
/// This is a combination of Tauri's [`generate_handler`](tauri::generate_handler) and Specta's [`collect_functions`](specta::function),
/// returning a [`Commands`](crate::Commands) struct that can be passed to [`Builder::commands`](crate::Builder::commands).
///
/// # Usage
/// ```rust,ignore-windows
/// use tauri_specta::{collect_commands,Builder};
///
/// #[tauri::command]
/// #[specta::specta] // < You must annotate your commands
/// fn hello_world(my_name: String) -> String {
///     format!("Hello, {my_name}! You've been greeted from Rust!")
/// }
///
/// #[tauri::command]
/// #[specta::specta] // < You must annotate your commands
/// fn generic_command<R: tauri::Runtime>(my_name: tauri::AppHandle<R>) -> String {
///     format!("You've been greeted from a generic Rust function!")
/// }
///
/// mod hello {
///     #[tauri::command]
///     #[specta::specta] // < You must annotate your commands
///     pub fn world() -> String {
///         format!("Hello world")
///     }
/// }
///
/// let mut builder = Builder::<tauri::Wry>::new()
///     .commands(collect_commands![
///         // You can pass a function name.
///         hello_world,
///         // You can also pass a module.
///         hello::world,
///         // Unlike `tauri::generate_handler` you may need to specify generics.
///         generic_command::<tauri::Wry>
///     ]);
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

/// Collect queries for TanStack Query integration.
///
/// Like [`collect_commands!`] but marks functions as queries, generating
/// `queryOptions` wrappers in TypeScript output.
///
/// # Usage
/// ```rust,ignore
/// use tauri_specta::{collect_queries, Builder};
///
/// #[tauri::command]
/// #[specta::specta]
/// fn get_user(id: u32) -> User { /* ... */ }
///
/// let builder = Builder::<tauri::Wry>::new()
///     .queries(collect_queries![get_user]);
/// ```
#[macro_export]
macro_rules! collect_queries {
    ($($b:ident $(:: $($p:ident)? $(<$($g:path),*>)? )* ),* $(,)?) => {
        $crate::internal::query(
            ::tauri::generate_handler![$($b $($(::$p)? )* ),*],
            ::specta::function::collect_functions![$($b $($(::$p)? $(::<$($g),*>)? )* ),*],
        )
    };
}

/// Collect mutations for TanStack Query integration.
///
/// Like [`collect_commands!`] but marks functions as mutations, generating
/// `mutationOptions` wrappers in TypeScript output.
///
/// # Usage
/// ```rust,ignore
/// use tauri_specta::{collect_mutations, Builder};
///
/// #[tauri::command]
/// #[specta::specta]
/// fn create_user(name: String) -> User { /* ... */ }
///
/// let builder = Builder::<tauri::Wry>::new()
///     .mutations(collect_mutations![create_user]);
/// ```
#[macro_export]
macro_rules! collect_mutations {
    ($($b:ident $(:: $($p:ident)? $(<$($g:path),*>)? )* ),* $(,)?) => {
        $crate::internal::mutation(
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
/// ```rust,ignore-windows
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
#[macro_export]
macro_rules! collect_events {
    ($($event:path),* $(,)?) => {{
        let mut events: $crate::Events = ::core::default::Default::default();
        $($crate::internal::register_event::<$event>(&mut events);)*
        events
    }};
}
