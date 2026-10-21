//! Regression coverage for https://github.com/specta-rs/tauri-specta/issues/172.
//!
//! With `CommandAccessors::ModuleIndexed` the generated frontend accessor is prefixed with
//! the module the command was registered from, so a command can keep a short Rust name
//! instead of encoding its module in the function name.

#![allow(missing_docs, dead_code, clippy::unwrap_used)]
#![cfg(all(feature = "typescript", feature = "derive"))]

use std::fs;

use specta_typescript::Typescript;
use tauri::test::MockRuntime;
use tauri_specta::{Builder, Casing, CommandAccessors, collect_commands};

// Commands are non-`pub` at the crate root so Tauri's generated macros don't collide.

#[tauri::command]
#[specta::specta]
fn root_command() -> String {
    "root".to_string()
}

mod some_module {
    #[tauri::command]
    #[specta::specta]
    pub fn command() -> String {
        "module".to_string()
    }

    #[tauri::command(rename = "renamed_command")]
    #[specta::specta]
    pub fn renamed() -> String {
        "renamed".to_string()
    }
}

mod outer {
    pub mod inner {
        #[tauri::command]
        #[specta::specta]
        pub fn deep() -> String {
            "deep".to_string()
        }
    }
}

// ----- harness -----

fn export(builder: &Builder<MockRuntime>) -> String {
    let dir = std::env::temp_dir().join(format!(
        "tauri_specta_module_indexed_{}_{:?}",
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
fn flat_accessors_are_the_default() {
    let builder = Builder::<MockRuntime>::new().commands(collect_commands![some_module::command]);

    let bindings = export(&builder);
    assert!(
        bindings.contains(r#"command: () => __TAURI_INVOKE<string>("command")"#),
        "{bindings}"
    );
}

#[test]
fn module_indexed_accessor_is_prefixed_with_the_module() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![root_command, some_module::command])
        .command_accessors(CommandAccessors::ModuleIndexed);

    let bindings = export(&builder);
    // A command registered from the crate root has no prefix.
    assert!(
        bindings.contains(r#"rootCommand: () => __TAURI_INVOKE<string>("root_command")"#),
        "{bindings}"
    );
    assert!(
        bindings.contains(r#"someModuleCommand: () => __TAURI_INVOKE<string>("command")"#),
        "{bindings}"
    );
}

#[test]
fn module_indexed_accessor_uses_the_whole_module_path() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![outer::inner::deep])
        .command_accessors(CommandAccessors::ModuleIndexed);

    let bindings = export(&builder);
    assert!(
        bindings.contains(r#"outerInnerDeep: () => __TAURI_INVOKE<string>("deep")"#),
        "{bindings}"
    );
}

#[test]
fn module_indexed_accessor_follows_function_casing() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![some_module::command])
        .function_casing(Casing::SnakeCase)
        .command_accessors(CommandAccessors::ModuleIndexed);

    let bindings = export(&builder);
    assert!(
        bindings.contains(r#"some_module_command: () => __TAURI_INVOKE<string>("command")"#),
        "{bindings}"
    );
}

#[test]
fn module_indexed_accessor_keeps_the_renamed_ipc_name() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![some_module::renamed])
        .command_accessors(CommandAccessors::ModuleIndexed);

    let bindings = export(&builder);
    // The accessor follows the Rust identifier (prefixed by the module), the IPC name is the
    // `rename`.
    assert!(
        bindings.contains(r#"someModuleRenamed: () => __TAURI_INVOKE<string>("renamed_command")"#),
        "{bindings}"
    );

    let response = invoke(&builder, "renamed_command").expect("the renamed name must be accepted");
    assert_eq!(response.deserialize::<String>().unwrap(), "renamed");
}
