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
    // Hide distracting implementation details from the generated rustdoc.
    ($($t:tt)*) => {
        $crate::collect_commands_internal!([] [] $($t)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! collect_commands_internal {
    () => {
        $crate::internal::command(
            ::tauri::generate_handler![],
            ::specta::function::collect_functions![],
        )
    };
    // Alternate parsing mode between `<` and `>` where all chars are not put into `stripped` accumulator
    ([$($stripped:tt)*] [$($raw:tt)*] []) => {
        compile_error!("Unexpected end of input. Did you forget to close a generic argument?");
    };
    ([$($stripped:tt)*] [$($raw:tt)*] [] > $($rest:tt)*) => {
        // Switch back to regular parsing mode
        $crate::collect_commands_internal!([$($stripped)*] [$($raw)* >] $($rest)*)
    };
    ([$($stripped:tt)*] [$($raw:tt)*] [] $a:tt $($rest:tt)*) => {
        $crate::collect_commands_internal!([$($stripped)*] [$($raw)* $a] [] $($rest)*)
    };
    // Regular parsing mode
    ([$($stripped:tt)*] [$($raw:tt)*]) => {
        $crate::internal::command(
            ::tauri::generate_handler![$($stripped)*],
            ::specta::function::collect_functions![$($raw)*],
        )
    };
    ([$($stripped:tt)*] [$($raw:tt)*] ::< $($rest:tt)*) => {
        // Switch to alternate parsing mode
        $crate::collect_commands_internal!([$($stripped)*] [$($raw)* ::<] [] $($rest)*)
    };
    ([$($stripped:tt)*] [$($raw:tt)*] $a:tt $($rest:tt)*) => {
        $crate::collect_commands_internal!([$($stripped)* $a] [$($raw)* $a] $($rest)*)
    };
    // Input
    ($($rest:tt)*) => {
        $crate::collect_commands_internal!([] [] $($rest)*);
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
