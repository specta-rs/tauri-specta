#![cfg(all(feature = "typescript", feature = "derive"))]
#![allow(missing_docs, clippy::unwrap_used)]

use std::fs;

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri::test::MockRuntime;
use tauri_specta::{Builder, Casing, Event, collect_commands, collect_events};

#[tauri::command]
#[specta::specta]
fn hello_world(my_name: String) -> String {
    format!("Hello, {my_name}!")
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
struct MyDemoEvent(String);

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
            r#"helloWorld: (myName: string) => __TAURI_INVOKE<string>("hello_world", { myName })"#
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
            r#"hello_world: (myName: string) => __TAURI_INVOKE<string>("hello_world", { myName })"#
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
            r#""hello-world": (myName: string) => __TAURI_INVOKE<string>("hello_world", { myName })"#
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
