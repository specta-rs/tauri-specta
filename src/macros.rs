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
#[macro_export]
macro_rules! collect_commands {
    ($($b:tt $(:: $($p:ident)? $(<$g:path>)? )* ),*) => {
        // We strip generics (::<...>) from being parsed to Tauri as it doesn't support them.
        $crate::internal::command(
            ::tauri::generate_handler![$($b $($(::$p)? )* ),*],
            ::specta::function::collect_functions![$($b $($(::$p)? $(::<$g>)? )* ),*],
        )
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
#[macro_export]
macro_rules! collect_events {
    ($($event:path),* $(,)?) => {{
        let mut events: $crate::Events = ::core::default::Default::default();
        $($crate::internal::register_event::<$event>(&mut events);)*
        events
    }};
}
