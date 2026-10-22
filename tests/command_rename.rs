//! Regression coverage for https://github.com/specta-rs/tauri-specta/issues/260.
//!
//! `#[tauri::command(rename = "...")]` changes the name Tauri's invoke handler matches
//! against, but `#[specta::specta]` names the function after its Rust identifier. These
//! tests guard that the generated bindings invoke the renamed name while the frontend
//! accessor still follows the Rust identifier.

#![allow(missing_docs, dead_code, clippy::unwrap_used)]
#![cfg(all(feature = "typescript", feature = "derive"))]

use std::fs;

use specta_typescript::Typescript;
use tauri::test::MockRuntime;
use tauri_specta::{Builder, Casing, collect_commands};

// Commands are non-`pub` at the crate root: a `pub fn` here exports Tauri's generated
// macros at the crate root of the test crate, which collides with the other commands.

#[tauri::command(rename = "renamed_greet")]
#[specta::specta]
fn greet() -> String {
    "hi".to_string()
}

#[tauri::command(rename = "renamed_greet_world")]
#[specta::specta]
fn greet_world() -> String {
    "hi".to_string()
}

#[tauri::command(rename = "renamed_generic")]
#[specta::specta]
fn generic_command<R: tauri::Runtime>(_app: tauri::AppHandle<R>) -> String {
    "generic".to_string()
}

#[tauri::command]
#[specta::specta]
fn unrenamed() -> String {
    "plain".to_string()
}

mod nested {
    #[tauri::command(rename = "nested_renamed")]
    #[specta::specta]
    pub fn nested_command() -> String {
        "nested".to_string()
    }
}

mod outer {
    pub mod inner {
        #[tauri::command(rename = "deep_renamed")]
        #[specta::specta]
        pub fn deep_command() -> String {
            "deep".to_string()
        }
    }
}

// ----- harness -----

fn export(builder: &Builder<MockRuntime>) -> String {
    let dir = std::env::temp_dir().join(format!(
        "tauri_specta_command_rename_{}_{:?}",
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

// ----- tests -----

#[test]
fn renamed_command_exports_renamed_ipc_name() {
    let bindings = export(&Builder::<MockRuntime>::new().commands(collect_commands![greet]));

    assert!(
        bindings.contains(r#"greet: () => __TAURI_INVOKE<string>("renamed_greet")"#),
        "{bindings}"
    );
}

#[test]
fn renamed_command_is_invokable_under_renamed_name() {
    let builder = Builder::<MockRuntime>::new().commands(collect_commands![greet]);

    let response = invoke(&builder, "renamed_greet").expect("the renamed name must be accepted");
    assert_eq!(response.deserialize::<String>().unwrap(), "hi");

    assert!(
        invoke(&builder, "greet").is_err(),
        "the original Rust name must be rejected"
    );
}

#[test]
fn accessor_follows_rust_identifier_and_function_casing() {
    let camel = Builder::<MockRuntime>::new()
        .commands(collect_commands![greet_world])
        .function_casing(Casing::CamelCase);
    let bindings = export(&camel);
    assert!(
        bindings.contains(r#"greetWorld: () => __TAURI_INVOKE<string>("renamed_greet_world")"#),
        "{bindings}"
    );

    let snake = Builder::<MockRuntime>::new()
        .commands(collect_commands![greet_world])
        .function_casing(Casing::SnakeCase);
    let bindings = export(&snake);
    assert!(
        bindings.contains(r#"greet_world: () => __TAURI_INVOKE<string>("renamed_greet_world")"#),
        "{bindings}"
    );
}

#[test]
fn generic_command_with_rename() {
    let builder =
        Builder::<MockRuntime>::new().commands(collect_commands![generic_command::<tauri::Wry>]);

    let bindings = export(&builder);
    assert!(
        bindings.contains(r#"genericCommand: () => __TAURI_INVOKE<string>("renamed_generic")"#),
        "{bindings}"
    );

    let response = invoke(&builder, "renamed_generic").expect("the renamed name must be accepted");
    assert_eq!(response.deserialize::<String>().unwrap(), "generic");
}

#[test]
fn command_in_submodule_with_rename() {
    let builder = Builder::<MockRuntime>::new().commands(collect_commands![nested::nested_command]);

    let bindings = export(&builder);
    assert!(
        bindings.contains(r#"nestedCommand: () => __TAURI_INVOKE<string>("nested_renamed")"#),
        "{bindings}"
    );

    let response = invoke(&builder, "nested_renamed").expect("the renamed name must be accepted");
    assert_eq!(response.deserialize::<String>().unwrap(), "nested");
}

#[test]
fn deeply_nested_command_with_rename() {
    let builder =
        Builder::<MockRuntime>::new().commands(collect_commands![outer::inner::deep_command]);

    let bindings = export(&builder);
    assert!(
        bindings.contains(r#"deepCommand: () => __TAURI_INVOKE<string>("deep_renamed")"#),
        "{bindings}"
    );

    let response = invoke(&builder, "deep_renamed").expect("the renamed name must be accepted");
    assert_eq!(response.deserialize::<String>().unwrap(), "deep");
}

#[test]
fn command_without_rename_still_exports_rust_name() {
    let builder = Builder::<MockRuntime>::new().commands(collect_commands![unrenamed]);

    let bindings = export(&builder);
    assert!(
        bindings.contains(r#"unrenamed: () => __TAURI_INVOKE<string>("unrenamed")"#),
        "{bindings}"
    );

    let response = invoke(&builder, "unrenamed").expect("the Rust name must be accepted");
    assert_eq!(response.deserialize::<String>().unwrap(), "plain");
}

#[test]
fn mock_handler_is_keyed_by_accessor_and_dispatched_by_renamed_name() {
    // The handler map is frontend-facing, so it is keyed by the accessor the Rust
    // identifier produces. The switch has to match the name the invoke handler receives
    // instead, or a mocked renamed command would never be found.
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![greet, unrenamed])
        .mocks();

    let bindings = export(&builder);
    assert!(
        bindings.contains("greet: () => string | Promise<string>,"),
        "the handler should be keyed by the frontend accessor:\n{bindings}"
    );
    assert!(
        bindings.contains(
            r#"case "renamed_greet": return mockCommandHandler(mockHandlers.greet, "renamed_greet", args);"#
        ),
        "the switch should match the renamed IPC name:\n{bindings}"
    );
    assert!(
        bindings.contains(
            r#"case "unrenamed": return mockCommandHandler(mockHandlers.unrenamed, "unrenamed", args);"#
        ),
        "{bindings}"
    );
}
