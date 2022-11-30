#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::Serialize;
use specta::{specta, Type};
use tauri_specta::{collate_types, export_to_openapi, export_to_ts};

#[tauri::command]
#[specta]
fn greet(name: String) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
#[specta]
fn greet2(name: String) -> impl Serialize + Type {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Serialize, Type)] // For Specta support you must add the `specta::Type` derive macro.
pub struct MyStruct {
    some_field: String,
}

#[tauri::command]
#[specta]
fn greet3() -> MyStruct {
    MyStruct {
        some_field: "Hello World".into(),
    }
}

fn main() {
    // Theses could be exported anywhere.
    // We found that a unit test for CI and to export on startup when debug assertions are enabled worked well for Spacedrive.

    // Specta provides the ability to introspect Rust types. This means someone could theoretically build a type exporter for any language into their own code or an external package.
    // I am going to be working on the ability to export to Rust in the near future for rspc and also working on making the OpenAPI exporter more robust (right now it will encounter issues with Rust generics).

    // Would be great if this was integrated directly into Tauri! collate_types and tauri_specta::command could be done away with.

    export_to_ts(collate_types![greet, greet2, greet3], "../src/bindings.ts").unwrap();

    export_to_openapi(collate_types![greet, greet2, greet3], "../src/openapi.json").unwrap();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet, greet2, greet3])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
