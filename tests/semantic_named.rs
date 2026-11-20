//! Regression coverage for semantic rules on top-level named command types.

#![cfg(all(feature = "typescript", feature = "javascript"))]

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::{JSDoc, Typescript, define, semantic};
use tauri::test::MockRuntime;
use tauri_specta::{Builder, collect_commands};

#[derive(Serialize, Deserialize, Type)]
struct SemanticBytes(Vec<u8>);

#[tauri::command]
fn echo_bytes(input: SemanticBytes) -> SemanticBytes {
    input
}

#[test]
fn named_semantic_types_transform_top_level_command_values() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![echo_bytes])
        .semantic_types(semantic::Configuration::empty().define::<SemanticBytes>(
            |_| define("Uint8Array").into(),
            Some(semantic::Transform::new(|v| format!("[...{v}]"))),
            Some(semantic::Transform::new(|v| format!("new Uint8Array({v})"))),
        ));

    let dir = std::env::temp_dir().join(format!(
        "tauri-specta-semantic-named-{}",
        std::process::id()
    ));
    let ts_path = dir.join("bindings.ts");
    let js_path = dir.join("bindings.js");
    builder
        .export(Typescript::default(), &ts_path)
        .expect("export TypeScript bindings");
    builder
        .export(JSDoc::default(), &js_path)
        .expect("export JavaScript bindings");

    let ts = std::fs::read_to_string(ts_path).expect("read TypeScript bindings");
    let js = std::fs::read_to_string(js_path).expect("read JavaScript bindings");
    std::fs::remove_dir_all(dir).expect("remove test exports");

    for bindings in [ts, js] {
        assert!(
            bindings.contains(r#""echo_bytes", { input: [...input] }, options"#),
            "{bindings}"
        );
        assert!(bindings.contains("new Uint8Array(v)"), "{bindings}");
    }
}
