//! Regression coverage for generated command parameter names and IPC keys.

#![cfg(all(feature = "typescript", feature = "javascript"))]
#![allow(clippy::unwrap_used)]

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::{JSDoc, Typescript, define, semantic};
use tauri::test::MockRuntime;
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands};

#[tauri::command(rename_all = "snake_case")]
fn tricky_arguments(
    class: String,
    foo_1: String,
    foo1: String,
    typed_error: String,
    _1st: String,
    options: String,
) -> Result<String, String> {
    Ok(class + &foo_1 + &foo1 + &typed_error + &_1st + &options)
}

#[derive(Deserialize, Serialize, Type)]
struct SemanticUrl(String);

#[derive(Deserialize, Serialize, Type)]
struct Payload {
    url: SemanticUrl,
}

#[tauri::command]
fn channel_argument(map_channel: String, on_progress: tauri::ipc::Channel<Payload>) {
    let _ = (map_channel, on_progress);
}

#[tauri::command]
fn raw_argument(_1st: Payload, request: tauri::ipc::Request<'_>) {
    let _ = (_1st, request);
}

#[allow(non_snake_case)]
#[tauri::command]
fn duplicate_payload_key(my_name: String, myName: String) {
    let _ = (my_name, myName);
}

#[allow(non_snake_case)]
#[tauri::command]
fn non_payload_key_collision(
    my_request: tauri::ipc::Request<'_>,
    myRequest: String,
    my_state: tauri::State<'_, ()>,
    myState: String,
) {
    let _ = (my_request, myRequest, my_state, myState);
}

#[test]
fn locals_are_valid_and_distinct_without_renaming_ipc_keys() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![tricky_arguments])
        .error_handling(ErrorHandlingMode::Result)
        .mocks();
    let dir = std::path::Path::new("target/tests/command-argument-names");
    let ts = dir.join("bindings.ts");
    let js = dir.join("bindings.js");
    builder.export(Typescript::default(), &ts).unwrap();
    builder.export(JSDoc::default(), &js).unwrap();
    let ts = std::fs::read_to_string(ts).unwrap();
    let js = std::fs::read_to_string(js).unwrap();

    assert!(
        ts.contains("trickyArguments: (_class: string, foo1: string, _foo1: string, _typedError: string, _1st: string, options: string, _options?: InvokeOptions) => typedError<string, string>"),
        "{ts}"
    );
    assert!(
        js.contains("trickyArguments: (_class, foo1, _foo1, _typedError, _1st, options, _options) => typedError("),
        "{js}"
    );
    for out in [&ts, &js] {
        assert!(
            out.contains(r#""tricky_arguments", { class: _class, foo_1: foo1, foo1: _foo1, typed_error: _typedError, "1st": _1st, options }, _options"#),
            "{out}"
        );
        assert!(
            out.contains(r#"args: { class: string; foo_1: string; foo1: string; typed_error: string; "1st": string; options: string }"#),
            "{out}"
        );
    }
}

#[test]
fn transforms_use_safe_locals_and_original_payload_keys() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![channel_argument, raw_argument])
        .semantic_types(semantic::Configuration::empty().define::<SemanticUrl>(
            |_| define("URL").into(),
            Some(semantic::Transform::new(|value| {
                format!("{value}.toString()")
            })),
            Some(semantic::Transform::new(|value| {
                format!("new URL({value})")
            })),
        ));
    let dir = std::path::Path::new("target/tests/command-argument-names");
    let ts = dir.join("transforms.ts");
    let js = dir.join("transforms.js");
    builder.export(Typescript::default(), &ts).unwrap();
    builder.export(JSDoc::default(), &js).unwrap();
    let ts = std::fs::read_to_string(ts).unwrap();
    let js = std::fs::read_to_string(js).unwrap();

    for out in [&ts, &js] {
        assert!(
            out.contains("mapChannel: _mapChannel, onProgress: mapChannel(onProgress,"),
            "{out}"
        );
        assert!(out.contains(r#"{ ...value, "1st":"#), "{out}");
        assert!(out.contains(r#"value["1st"]"#), "{out}");
    }
}

#[test]
fn rejects_duplicate_payload_keys() {
    let builder = Builder::<MockRuntime>::new().commands(collect_commands![duplicate_payload_key]);
    let dir = std::path::Path::new("target/tests/command-argument-names");

    for (builder, stem) in [
        (builder.clone(), "duplicate-payload"),
        (builder.mocks(), "duplicate-payload-mocks"),
    ] {
        let results = [
            builder.export(Typescript::default(), dir.join(format!("{stem}.ts"))),
            builder.export(JSDoc::default(), dir.join(format!("{stem}.js"))),
        ];
        for result in results {
            let error = result
                .expect_err("duplicate wire keys must not generate bindings")
                .to_string();
            assert!(
                error.contains("Command 'duplicate_payload_key' uses the IPC payload key 'myName' more than once"),
                "{error}"
            );
        }
    }
}

#[test]
fn ignores_non_payload_arguments_when_checking_keys() {
    let builder =
        Builder::<MockRuntime>::new().commands(collect_commands![non_payload_key_collision]);
    let dir = std::path::Path::new("target/tests/command-argument-names");
    let ts = dir.join("non-payload.ts");
    let js = dir.join("non-payload.js");
    builder.export(Typescript::default(), &ts).unwrap();
    builder.export(JSDoc::default(), &js).unwrap();
    for path in [ts, js] {
        let out = std::fs::read_to_string(path).unwrap();
        assert!(
            out.contains("{ myRequest: string; myState: string }"),
            "{out}"
        );
    }
}
