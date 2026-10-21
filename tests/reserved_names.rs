//! Regression coverage for names the generated runtime declares.
//!
//! The TypeScript runtime declares `EventEmit`/`EventEmitTo`; the JSDoc runtime
//! does not, so the reserved-name check has to stay emitter-aware.
//!
//! https://github.com/specta-rs/tauri-specta/issues/187

#![cfg(all(feature = "typescript", feature = "derive"))]
#![allow(missing_docs, clippy::unwrap_used)]

use std::fs;

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::{JSDoc, Typescript};
use tauri::test::MockRuntime;
use tauri_specta::{Builder, LanguageExt};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct EventEmit {
    value: String,
}

fn export<L: LanguageExt>(
    builder: &Builder<MockRuntime>,
    language: L,
    file_name: &str,
) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "tauri_specta_reserved_names_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&dir).unwrap();
    builder
        .export(language, dir.join(file_name))
        .map_err(|err| err.to_string())
}

#[test]
fn jsdoc_allows_types_named_after_typescript_only_runtime_helpers() {
    let builder = Builder::<MockRuntime>::new().typ::<EventEmit>();
    export(&builder, JSDoc::default(), "bindings.js")
        .expect("the JSDoc runtime has no `EventEmit` typedef for this name to collide with");
}

#[test]
fn typescript_rejects_types_named_after_its_runtime_helpers() {
    let builder = Builder::<MockRuntime>::new().typ::<EventEmit>();
    let err = export(&builder, Typescript::default(), "bindings.ts")
        .expect_err("the TypeScript runtime declares `EventEmit`");
    assert!(err.contains("must be renamed"), "unexpected error: {err}");
}
