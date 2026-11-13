//! Regression tests for raw IPC binding types.
#![cfg(all(feature = "typescript", feature = "derive"))]
#![allow(missing_docs, dead_code, clippy::unwrap_used)]

use tauri::test::MockRuntime;
use tauri_specta::{Builder, LanguageExt, collect_commands};

#[tauri::command]
#[specta::specta]
fn ipc(request: tauri::ipc::Request<'_>) -> tauri::ipc::Response {
    let _ = request;
    tauri::ipc::Response::new(Vec::<u8>::new())
}

#[tauri::command]
#[specta::specta]
fn fallible_ipc() -> Result<tauri::ipc::Response, String> {
    Ok(tauri::ipc::Response::new("null".to_string()))
}

#[tauri::command]
#[specta::specta]
fn mixed(todo: String, request: tauri::ipc::Request<'_>) -> String {
    let _ = request;
    todo
}

#[tauri::command]
#[specta::specta]
fn no_args() {}

#[tauri::command]
#[specta::specta]
fn option_names(options: String, invoke_options: String) {
    let _ = (options, invoke_options);
}

fn export<L: LanguageExt>(builder: &Builder<MockRuntime>, lang: L, suffix: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "tauri-specta-ipc-types-{}-{suffix}",
        std::process::id()
    ));
    builder.export(lang, &path).unwrap();
    let output = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    output
}

#[test]
fn raw_ipc_uses_sdk_request_types_and_unknown_response() {
    let builder =
        Builder::<MockRuntime>::new().commands(collect_commands![ipc, fallible_ipc, mixed]);
    let ts = export(
        &builder,
        specta_typescript::Typescript::default(),
        "bindings.ts",
    );
    assert!(
        ts.contains("import type { InvokeArgs, InvokeOptions }"),
        "{ts}"
    );
    assert!(
        ts.contains("request: InvokeArgs, options?: InvokeOptions"),
        "{ts}"
    );
    assert!(
        ts.contains("__TAURI_INVOKE<unknown>(\"ipc\", Array.isArray(request) ? new Uint8Array(request) : request, options)"),
        "{ts}"
    );
    assert!(
        !ts.contains("TAURI_REQUEST") && !ts.contains("TAURI_RESPONSE"),
        "{ts}"
    );
    assert!(!ts.contains("__TAURI_INVOKE<ArrayBuffer>"), "{ts}");
    assert!(ts.contains("mixed: (value: { todo: string }, options?: InvokeOptions) => __TAURI_INVOKE<string>(\"mixed\", value, options)"), "{ts}");

    let js = export(&builder, specta_typescript::JSDoc::default(), "bindings.js");
    assert!(
        js.contains("Array.isArray(request) ? new Uint8Array(request) : request"),
        "{js}"
    );
    assert!(js.contains("@param {InvokeArgs} request"), "{js}");
    assert!(js.contains("@returns {Promise<unknown>}"), "{js}");
    assert!(js.contains("@param {{ todo: string }} value"), "{js}");
    assert!(
        js.contains("__TAURI_INVOKE(\"mixed\", value, options)"),
        "{js}"
    );
    assert!(
        !js.contains("TAURI_REQUEST") && !js.contains("TAURI_RESPONSE"),
        "{js}"
    );
}

#[test]
fn options_are_available_for_ordinary_and_plugin_commands() {
    let builder = Builder::<MockRuntime>::new()
        .plugin_name("sample")
        .commands(collect_commands![no_args, option_names]);
    let ts = export(
        &builder,
        specta_typescript::Typescript::default(),
        "options.ts",
    );
    assert!(ts.contains("import type { InvokeOptions }"), "{ts}");
    assert!(ts.contains("noArgs: (options?: InvokeOptions) => __TAURI_INVOKE<void>(\"plugin:sample|no_args\", undefined, options)"), "{ts}");
    assert!(
        ts.contains("options: string, invokeOptions: string, _options?: InvokeOptions"),
        "{ts}"
    );
    assert!(
        ts.contains("\"plugin:sample|option_names\", { options, invokeOptions }, _options"),
        "{ts}"
    );
    let js = export(&builder, specta_typescript::JSDoc::default(), "options.js");
    assert!(js.contains("@param {InvokeOptions} [options]"), "{js}");
    assert!(
        js.contains("__TAURI_INVOKE(\"plugin:sample|no_args\", undefined, options)"),
        "{js}"
    );
}
