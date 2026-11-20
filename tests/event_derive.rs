//! Integration tests for the event derive.

#![cfg(feature = "derive")]
// The raw-identifier fixtures are intentionally lower case and their fields are only
// read through `Event::NAME`.
#![allow(dead_code, non_camel_case_types)]

use std::marker::PhantomData;

use specta::Type;
use tauri_specta::{Builder, Event, collect_events};

#[derive(Type, Event)]
struct GenericEvent<T>(T);

// A raw identifier is still the type's real name: `r#type` names `type`, so the
// derived event name must not contain the `r#` escape.
#[derive(Type, Event)]
struct r#type(String);

#[derive(Type, Event)]
enum r#enum {
    Value,
}

#[derive(Clone)]
struct Marker;

#[derive(Type, Event)]
struct MarkerEvent<T = Marker>
where
    T: Clone,
{
    #[specta(skip)]
    marker: PhantomData<T>,
}

#[test]
fn raw_identifiers_use_the_unrawed_name() {
    assert_eq!(<r#type as Event>::NAME, "type");
    assert_eq!(<r#enum as Event>::NAME, "enum");
}

#[test]
fn generic_event_derives_without_explicit_bounds() {
    let _ =
        Builder::<tauri::test::MockRuntime>::new().events(collect_events![GenericEvent<String>]);
    assert_eq!(GenericEvent::<String>::NAME, "generic-event");
}

#[test]
fn skipped_event_parameter_does_not_require_type() {
    assert_eq!(MarkerEvent::<Marker>::NAME, "marker-event");
    let _ = collect_events![MarkerEvent];
}
