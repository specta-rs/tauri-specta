/// Collect commands and their types.
///
/// This is a combination of Tauri's [`generate_handler`](tauri::generate_handler) and Specta's [`collect_functions`](specta::function::collect_functions).
///
/// This returns a [`Commands`](crate::Commands) struct that can be passed to [`Builder::commands`](crate::Builder::commands).
///
/// # Usage
/// ```rust
/// collect_commands![];
/// collect_commands![hello_world];
/// collect_commands![hello_world, some::path::function, generic_fn::<String>];
/// ```
///
// TODO: Hide it's implementation details from the generated rustdoc.
#[macro_export]
macro_rules! collect_commands {
    () => {
        $crate::internal::command(::tauri::generate_handler![], ::specta::function::collect_functions![])
    };
    ($i:ident) => {
        $crate::internal::command(::tauri::generate_handler![$i], ::specta::function::collect_functions![$i])
    };
    ($i:ident, $($rest:tt)*) => {
        $crate::internal::command($crate::collect_commands!(@internal; $i; $($rest)*), ::specta::function::collect_functions![$i, $($rest)*])
    };
    ($i:ident::<$g:path>) => {
        $crate::internal::command(::tauri::generate_handler![$i], ::specta::function::collect_functions![$i<$g>])
    };
    ($i:ident::<$g:path>, $($rest:tt)*) => {
        $crate::internal::command($crate::collect_commands!(@internal; $i; $($rest)*), ::specta::function::collect_functions![$i<$g>, $($rest)*])
    };
    //
    (@internal; $($b:path),*;) => {
        ::tauri::generate_handler![$($b),*]
    };
    (@internal; $($b:path),*; $i:ident) => {
        ::tauri::generate_handler![$($b),*, $i]
    };
    (@internal; $($b:path),*; $i:ident, $($rest:tt)*) => {
        $crate::collect_commands!(@internal; $($b),*, $i; $($rest)*)
    };
    (@internal; $($b:path),*; $i:ident::<$g:path>) => {
        ::tauri::generate_handler![$($b),*, $i]
    };
    (@internal; $($b:path),*; $i:ident::<$g:ident>, $($rest:tt)*) => {
        $crate::collect_commands!(@internal; $($b),*, $i; $($rest)*)
    };
}

/// Collect events and their types.
///
/// This returns a [`Events`](crate::Events) struct that can be passed to [`Builder::events`](crate::Builder::events).
///
/// # Usage
/// ```rust
/// collect_events![];
/// collect_events![MyEvent];
/// collect_events![MyEvent, module::MyOtherEvent];
/// ```
///
// TODO: Hide it's implementation details from the generated rustdoc.
#[macro_export]
macro_rules! collect_events {
    ($($event:path),* $(,)?) => {{
        let mut events: $crate::Events = ::core::default::Default::default();
        $($crate::internal::register_event::<$event>(&mut events);)*
        events
    }};
}
