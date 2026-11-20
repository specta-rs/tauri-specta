#![cfg(all(feature = "typescript", feature = "derive"))]
#![allow(missing_docs, clippy::unwrap_used)]

use std::fs;

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri::test::MockRuntime;
use tauri_specta::{Builder, Casing, Event, collect_commands, collect_events};

#[tauri::command]
fn hello_world(my_name: String) -> String {
    format!("Hello, {my_name}!")
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
struct MyDemoEvent(String);

#[derive(Serialize, Deserialize, Type, Event)]
#[tauri_specta(event_name = "job-done")]
struct KebabEvent(String);

#[derive(Serialize, Deserialize, Type, Event)]
#[tauri_specta(event_name = "job_done")]
struct SnakeEvent(String);

fn export_to_string(builder: &Builder<MockRuntime>) -> String {
    let dir = std::env::temp_dir().join(format!(
        "tauri_specta_casing_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bindings.ts");
    builder
        .export(Typescript::default(), &path)
        .expect("failed to export bindings");
    fs::read_to_string(&path).unwrap()
}

#[test]
fn default_casing_is_camel_case() {
    let builder = Builder::<MockRuntime>::new().commands(collect_commands![hello_world]);
    let out = export_to_string(&builder);

    assert!(
        out.contains(
            r#"helloWorld: (myName: string, options?: InvokeOptions) => __TAURI_INVOKE<string>("hello_world", { myName }, options)"#
        ),
        "expected camelCase accessor by default, got:\n{out}"
    );
}

#[test]
fn snake_case_function_casing() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![hello_world])
        .function_casing(Casing::SnakeCase);
    let out = export_to_string(&builder);

    assert!(
        out.contains(
            r#"hello_world: (myName: string, options?: InvokeOptions) => __TAURI_INVOKE<string>("hello_world", { myName }, options)"#
        ),
        "expected snake_case accessor and unchanged arguments, got:\n{out}"
    );
}

#[test]
fn kebab_case_function_casing_uses_a_quoted_property() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![hello_world])
        .function_casing(Casing::KebabCase);
    let out = export_to_string(&builder);

    assert!(
        out.contains(
            r#""hello-world": (myName: string, options?: InvokeOptions) => __TAURI_INVOKE<string>("hello_world", { myName }, options)"#
        ),
        "expected a quoted kebab-case accessor, got:\n{out}"
    );
}

#[test]
fn function_casing_applies_to_events() {
    let builder = Builder::<MockRuntime>::new()
        .events(collect_events![MyDemoEvent])
        .function_casing(Casing::SnakeCase);
    let out = export_to_string(&builder);

    assert!(
        out.contains("my_demo_event: makeEvent"),
        "expected snake_case event accessor, got:\n{out}"
    );
}

#[test]
fn event_accessor_collisions_are_rejected() {
    let builder = Builder::<MockRuntime>::new().events(collect_events![KebabEvent, SnakeEvent]);
    let dir = std::env::temp_dir().join(format!(
        "tauri_specta_event_collision_{}",
        std::process::id()
    ));
    let err = builder
        .export(Typescript::default(), dir.join("bindings.ts"))
        .expect_err("the same accessor cannot represent two events");
    let message = err.to_string();
    assert!(
        message.contains("'job-done'")
            && message.contains("'job_done'")
            && message.contains("'jobDone'"),
        "{message}"
    );

    #[cfg(feature = "javascript")]
    assert_eq!(
        builder
            .export(specta_typescript::JSDoc::default(), dir.join("bindings.js"))
            .expect_err("JSDoc has the same event accessors")
            .to_string(),
        message
    );
}
