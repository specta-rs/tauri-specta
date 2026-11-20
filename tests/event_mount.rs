//! Integration tests for mounting typed events.

#![cfg(feature = "derive")]
#![allow(clippy::unwrap_used)]

use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use serde::Serialize;
use specta::Type;
use tauri::{App, Listener, test::MockRuntime};
use tauri_specta::{Builder, Event, collect_events};

#[derive(Clone, Serialize, Type, Event)]
struct Shared(u32);

#[derive(Clone, Serialize, Type, Event)]
struct Extra(u32);

fn count_deliveries(app: &App<MockRuntime>, name: &'static str) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let listener_count = count.clone();
    app.listen(name, move |_| {
        listener_count.fetch_add(1, Ordering::Relaxed);
    });
    count
}

fn panic_message(payload: &(dyn Any + Send)) -> &str {
    if let Some(message) = payload.downcast_ref::<String>() {
        message
    } else {
        payload.downcast_ref::<&str>().copied().unwrap()
    }
}

#[test]
fn rejects_a_conflicting_namespace_without_changing_the_registry() {
    let app = tauri::test::mock_app();
    let one = Builder::new()
        .plugin_name("one")
        .events(collect_events![Shared]);
    let two = Builder::new()
        .plugin_name("two")
        .events(collect_events![Shared, Extra]);
    let delivered = count_deliveries(&app, "plugin:one:shared");

    one.mount_events(&app);
    one.mount_events(&app);
    Shared(1).emit(&app).unwrap();

    let conflict = catch_unwind(AssertUnwindSafe(|| two.mount_events(&app)))
        .expect_err("the same event type must not mount under two plugins");
    let message = panic_message(conflict.as_ref());
    assert!(message.contains("plugin:one:shared"), "{message}");
    assert!(message.contains("plugin:two:shared"), "{message}");

    // The registry is still usable and the first namespace still receives the event.
    Shared(2).emit(&app).unwrap();
    assert_eq!(delivered.load(Ordering::Relaxed), 2);

    // Extra sorts before Shared. Even it must not be mounted by the rejected batch.
    let missing = catch_unwind(AssertUnwindSafe(|| Extra(3).emit(&app)))
        .expect_err("the rejected batch must not mount other event types");
    assert!(panic_message(missing.as_ref()).contains("Event extra not found in registry"));

    let extra_delivered = count_deliveries(&app, "plugin:two:extra");
    Builder::new()
        .plugin_name("two")
        .events(collect_events![Extra])
        .mount_events(&app);
    Extra(4).emit(&app).unwrap();
    assert_eq!(extra_delivered.load(Ordering::Relaxed), 1);
}

#[test]
fn application_and_plugin_namespaces_also_conflict() {
    let app = tauri::test::mock_app();
    Builder::new()
        .events(collect_events![Shared])
        .mount_events(&app);
    let plugin = Builder::new()
        .plugin_name("sample")
        .events(collect_events![Shared]);

    let conflict = catch_unwind(AssertUnwindSafe(|| plugin.mount_events(&app)))
        .expect_err("an app event must not be remounted as a plugin event");
    let message = panic_message(conflict.as_ref());
    assert!(message.contains("'shared'"), "{message}");
    assert!(message.contains("plugin:sample:shared"), "{message}");
}

#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "name-collision")]
struct NameCollision(u32);

#[derive(Clone, Serialize, Type, Event)]
#[tauri_specta(event_name = "name-collision")]
struct NameCollisionOther(String);

#[test]
fn rejects_two_event_types_that_share_a_tauri_name() {
    let app = tauri::test::mock_app();
    let first = Builder::new().events(collect_events![NameCollision]);
    let second = Builder::new().events(collect_events![NameCollisionOther]);
    let delivered = count_deliveries(&app, "name-collision");

    first.mount_events(&app);
    NameCollision(1).emit(&app).unwrap();

    // Both types resolve to the same Tauri event name in the same namespace, so a
    // listener typed for one would receive the other's payload.
    let conflict = catch_unwind(AssertUnwindSafe(|| second.mount_events(&app)))
        .expect_err("two event types must not share one Tauri event name");
    let message = panic_message(conflict.as_ref());
    assert!(message.contains("'name-collision'"), "{message}");

    // The rejected batch must not mount any of its types, and the first type still works.
    let missing = catch_unwind(AssertUnwindSafe(|| {
        NameCollisionOther(String::new()).emit(&app)
    }))
    .expect_err("the rejected batch must not mount its event types");
    assert!(panic_message(missing.as_ref()).contains("Event name-collision not found in registry"));
    NameCollision(2).emit(&app).unwrap();
    assert_eq!(delivered.load(Ordering::Relaxed), 2);
}

#[test]
fn the_same_name_may_be_used_in_a_different_namespace() {
    let app = tauri::test::mock_app();
    Builder::new()
        .events(collect_events![NameCollision])
        .mount_events(&app);

    // A distinct event type with the same name is fine under a plugin namespace: the
    // resolved Tauri names differ, so neither type can deliver to the other's listeners.
    let plugin = Builder::new()
        .plugin_name("sample")
        .events(collect_events![NameCollisionOther]);
    plugin.mount_events(&app);

    let delivered = count_deliveries(&app, "plugin:sample:name-collision");
    NameCollisionOther(String::new()).emit(&app).unwrap();
    assert_eq!(delivered.load(Ordering::Relaxed), 1);
}
