#![allow(missing_docs)]
#![cfg(feature = "typescript")]

use specta::Type;
use tauri_specta::{Builder, collect_commands};

#[derive(serde::Serialize, serde::Deserialize, Type)]
struct Thing {
    id: u32,
    name: String,
    tags: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Type)]
#[specta(inline)]
struct InlineThing {
    id: u32,
}

#[tauri::command]
#[specta::specta]
fn bare(thing: Thing) -> Thing {
    thing
}

#[tauri::command]
#[specta::specta]
fn optional(thing: Option<Thing>) -> Option<Thing> {
    thing
}

#[tauri::command]
#[specta::specta]
fn optional_inline(thing: Option<InlineThing>) -> Option<InlineThing> {
    thing
}

#[tauri::command]
#[specta::specta]
fn listed(thing: Vec<Thing>) -> Vec<Thing> {
    thing
}

#[tauri::command]
#[specta::specta]
fn resulted() -> Result<Option<Thing>, String> {
    unimplemented!()
}

// Regression coverage for https://github.com/specta-rs/tauri-specta/issues/228.
#[test]
fn named_types_inside_options_are_referenced() {
    let path = std::env::temp_dir().join(format!(
        "tauri-specta-option-type-references-{}.ts",
        std::process::id()
    ));

    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            bare,
            optional,
            optional_inline,
            listed,
            resulted
        ])
        .export(specta_typescript::Typescript::default(), &path)
        .expect("bindings export should succeed");

    let bindings = std::fs::read_to_string(&path).expect("bindings should be readable");
    std::fs::remove_file(path).expect("temporary bindings should be removable");

    assert!(bindings.contains("bare: (thing: Thing)"), "{bindings}");
    assert!(
        bindings.contains("optional: (thing: Thing | null)"),
        "{bindings}"
    );
    assert!(
        bindings.contains("__TAURI_INVOKE<Thing | null>(\"optional\""),
        "{bindings}"
    );
    assert!(
        bindings.contains("optionalInline: (thing: {\n\tid: number,\n} | null)"),
        "{bindings}"
    );
    assert!(bindings.contains("listed: (thing: Thing[])"), "{bindings}");
    assert!(
        bindings.contains("typedError<Thing | null, string>("),
        "{bindings}"
    );
}
