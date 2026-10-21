//! Regression coverage for https://github.com/specta-rs/tauri-specta/issues/113.
//!
//! `Builder::commands` and `Builder::events` accumulate across calls, so a builder can be
//! assembled from several `collect_commands!` / `collect_events!` groups. Every command
//! registered this way has to be dispatched by the resulting invoke handler, and
//! re-registering a name is rejected rather than silently keeping one of them.

#![allow(missing_docs, dead_code, clippy::unwrap_used)]
#![cfg(all(feature = "typescript", feature = "derive"))]

use std::fs;

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri::test::MockRuntime;
use tauri_specta::{Builder, Casing, CommandExports, Event, collect_commands, collect_events};

// Commands are non-`pub` at the crate root so Tauri's generated macros don't collide.

#[tauri::command]
#[specta::specta]
fn first() -> String {
    "first".to_string()
}

#[tauri::command]
#[specta::specta]
fn second() -> String {
    "second".to_string()
}

#[tauri::command]
#[specta::specta]
fn multi_word() -> String {
    "multi".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
struct FirstEvent(String);

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
struct SecondEvent(String);

// ----- harness -----

fn export(builder: &Builder<MockRuntime>) -> String {
    let dir = std::env::temp_dir().join(format!(
        "tauri_specta_registration_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bindings.ts");

    builder.export(Typescript::default(), &path).unwrap();
    let bindings = fs::read_to_string(&path).unwrap();

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&dir);

    bindings
}

fn invoke(
    builder: &Builder<MockRuntime>,
    cmd: &str,
) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
    let app = tauri::test::mock_builder()
        .invoke_handler(builder.invoke_handler())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    tauri::test::get_ipc_response(
        &webview,
        tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: tauri::ipc::InvokeBody::default(),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
}

// ----- commands -----

#[test]
fn commands_accumulate_across_calls() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![first])
        .commands(collect_commands![second]);

    let bindings = export(&builder);
    assert!(
        bindings.contains(r#"first: () => __TAURI_INVOKE<string>("first")"#),
        "{bindings}"
    );
    assert!(
        bindings.contains(r#"second: () => __TAURI_INVOKE<string>("second")"#),
        "{bindings}"
    );

    // Both commands must also be dispatched by the chained invoke handler.
    assert_eq!(
        invoke(&builder, "first")
            .unwrap()
            .deserialize::<String>()
            .unwrap(),
        "first"
    );
    assert_eq!(
        invoke(&builder, "second")
            .unwrap()
            .deserialize::<String>()
            .unwrap(),
        "second"
    );
}

#[test]
#[should_panic(expected = "Command 'first' is registered multiple times")]
fn duplicate_command_across_calls_panics() {
    let _ = Builder::<MockRuntime>::new()
        .commands(collect_commands![first])
        .commands(collect_commands![first]);
}

// ----- events -----

#[test]
fn events_accumulate_across_calls() {
    let builder = Builder::<MockRuntime>::new()
        .events(collect_events![FirstEvent])
        .events(collect_events![SecondEvent]);

    let bindings = export(&builder);
    assert!(bindings.contains("firstEvent: makeEvent<"), "{bindings}");
    assert!(bindings.contains("secondEvent: makeEvent<"), "{bindings}");
}

#[test]
#[should_panic(expected = "Event 'first-event' is registered multiple times")]
fn duplicate_event_across_calls_panics() {
    let _ = Builder::<MockRuntime>::new()
        .events(collect_events![FirstEvent])
        .events(collect_events![FirstEvent]);
}

// ----- top-level command exports -----

#[test]
fn top_level_exports_drop_the_commands_object() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![first, second])
        .command_exports(CommandExports::TopLevel);

    let bindings = export(&builder);
    assert!(!bindings.contains("export const commands"), "{bindings}");
    assert!(
        bindings.contains(r#"export const first = () => __TAURI_INVOKE<string>("first");"#),
        "{bindings}"
    );
}

#[test]
fn top_level_exports_reject_non_identifier_casing() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![multi_word])
        .function_casing(Casing::KebabCase)
        .command_exports(CommandExports::TopLevel);

    let err = builder
        .export(
            Typescript::default(),
            std::env::temp_dir().join("unused.ts"),
        )
        .expect_err("a non-identifier accessor should fail the export")
        .to_string();
    assert!(
        err.contains("not a valid JavaScript identifier"),
        "unexpected error: {err}"
    );
}
