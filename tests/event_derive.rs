//! Integration tests for the event derive.

#![cfg(feature = "derive")]

use std::marker::PhantomData;

use specta::Type;
use tauri_specta::{Builder, Event, collect_events};

#[derive(Type, Event)]
struct GenericEvent<T>(T);

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
