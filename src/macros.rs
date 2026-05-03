/// Collect commands and their types.
///
/// This is a combination of Tauri's [`generate_handler`](tauri::generate_handler) and Tauri Specta's command inference,
/// returning a [`Commands`](crate::Commands) struct that can be passed to [`Builder::commands`](crate::Builder::commands).
///
/// # Usage
/// ```rust,ignore-windows
/// use tauri_specta::{collect_commands,Builder};
///
/// #[tauri::command]
/// fn hello_world(my_name: String) -> String {
///     format!("Hello, {my_name}! You've been greeted from Rust!")
/// }
///
/// #[tauri::command]
/// fn generic_command<R: tauri::Runtime>(my_name: tauri::AppHandle<R>) -> String {
///     format!("You've been greeted from a generic Rust function!")
/// }
///
/// mod hello {
///     #[tauri::command]
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
            {
                fn export(types: &mut ::specta::Types) -> ::std::vec::Vec<$crate::Command> {
                    ::std::vec![
                        $(
                            $crate::internal::infer_command(
                                $b $($(::$p)? $(::<$($g),*>)? )*,
                                $crate::__private_command_definition!(($b $($(::$p)? $(::<$($g),*>)? )*)),
                                types,
                            )
                        ),*
                    ]
                }

                export
            },
        )
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __private_command_definition {
    (($($command:tt)+)) => {
        $crate::__private_command_definition_inner!([] $($command)+)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __private_command_definition_inner {
    ([$($prefix:tt)*] $name:ident $(::<$($g:path),*>)?) => {
        $crate::internal::paste! {
            $($prefix)* [<__cmd__ $name>]!(@definition)
        }
    };
    ([$($prefix:tt)*] $head:ident :: $($tail:tt)+) => {
        $crate::__private_command_definition_inner!([$($prefix)* $head ::] $($tail)+)
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
