#![cfg(all(feature = "typescript", feature = "derive"))]
#![allow(missing_docs, clippy::unwrap_used, clippy::panic)]

use std::fs;

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri::WebviewWindowBuilder;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri_specta::{Builder, Casing, Event, collect_commands, collect_events};

#[tauri::command]
#[specta::specta]
fn hello_world(my_name: String) -> String {
    format!("Hello, {my_name}!")
}

// `rename_all = "snake_case"` makes Tauri expect snake_case argument keys from the frontend.
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
fn greet_user(user_name: String) -> String {
    format!("Hello, {user_name}, from Rust!")
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

fn invoke_request(cmd: &str, body: serde_json::Value) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: if cfg!(any(windows, target_os = "android")) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        }
        .parse()
        .unwrap(),
        body: InvokeBody::from(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

#[test]
fn default_casing_is_camel_case() {
    let builder = Builder::<MockRuntime>::new().commands(collect_commands![hello_world]);
    let out = export_to_string(&builder);

    assert!(
        out.contains(
            r#"helloWorld: (myName: string) => __TAURI_INVOKE<string>("hello_world", { myName })"#
        ),
        "expected camelCase accessor and argument by default, got:\n{out}"
    );
}

#[test]
fn snake_case_function_and_argument_casing() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![hello_world])
        .function_casing(Casing::SnakeCase)
        .argument_casing(Casing::SnakeCase);
    let out = export_to_string(&builder);

    assert!(
        out.contains(
            r#"hello_world: (my_name: string) => __TAURI_INVOKE<string>("hello_world", { my_name })"#
        ),
        "expected snake_case accessor and argument, got:\n{out}"
    );
    // The underlying Tauri command string must be unchanged.
    assert!(out.contains(r#""hello_world""#));
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

// The argument keys emitted by the bindings must match what Tauri actually accepts.
// With `rename_all = "snake_case"`, Tauri expects snake_case keys, so the camelCase keys
// emitted by the default behavior would fail. This proves `argument_casing` fixes that.
#[test]
fn snake_case_arguments_match_tauri_rename_all() {
    let ts_builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![greet_user])
        .argument_casing(Casing::SnakeCase);
    let out = export_to_string(&ts_builder);
    assert!(
        out.contains(r#"__TAURI_INVOKE<string>("greet_user", { user_name })"#),
        "expected snake_case argument key, got:\n{out}"
    );

    let app_builder = Builder::<MockRuntime>::new().commands(collect_commands![greet_user]);
    let app = mock_builder()
        .invoke_handler(app_builder.invoke_handler())
        .build(mock_context(noop_assets()))
        .expect("failed to build mock app");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to build webview");

    // snake_case key (what the new bindings emit) succeeds.
    let ok = get_ipc_response(
        &webview,
        invoke_request("greet_user", serde_json::json!({ "user_name": "Cursor" })),
    );
    assert_eq!(
        ok.expect("snake_case invoke should succeed")
            .deserialize::<String>()
            .unwrap(),
        "Hello, Cursor, from Rust!"
    );

    // camelCase key (what the old default bindings emit) fails against a snake_case command.
    let err = get_ipc_response(
        &webview,
        invoke_request("greet_user", serde_json::json!({ "userName": "Cursor" })),
    );
    assert!(
        err.is_err(),
        "camelCase key should not satisfy a `rename_all = \"snake_case\"` command"
    );
}
