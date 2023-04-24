#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::Serialize;
use specta::{collect_types, specta, Type};
use tauri_specta::*;

/// HELLO
/// WORLD
/// !!!!
#[tauri::command]
#[specta]
fn hello_world(my_name: String) -> String {
    format!("Hello, {my_name}! You've been greeted from Rust!")
}

#[tauri::command]
#[specta]
fn goodbye_world() -> impl Serialize + Type {
    "Goodbye world :("
}

mod nested {
    use super::*;

    #[tauri::command]
    #[specta]
    pub fn some_struct() -> MyStruct {
        MyStruct {
            some_field: "Hello World".into(),
        }
    }

    #[derive(Serialize, Type)] // For Specta support you must add the `specta::Type` derive macro.
    pub struct MyStruct {
        some_field: String,
    }
}

fn main() {
    // Theses could be exported anywhere.
    // We found that a unit test for CI and to export on startup when debug assertions are enabled worked well for Spacedrive.

    // Specta provides the ability to introspect Rust types.
    // This means someone could theoretically build a type exporter for any language into their own code or an external package.
    // I am going to be working on the ability to export to Rust in the near future for rspc

    // Would be great if this was integrated directly into Tauri! collate_types and tauri_specta::command could be done away with.

    ts::export(
        collect_types![hello_world, goodbye_world, nested::some_struct],
        "../src/bindings.ts",
    )
    .unwrap();

    js::export(
        collect_types![hello_world, goodbye_world, nested::some_struct],
        "../src/bindings.js",
    )
    .unwrap();

    // This is useful for custom eslint, prettier overrides at the top of the file.
    // ts::export_with_cfg_with_header(
    //     collect_types![hello_world, goodbye_world, nested::some_struct].unwrap(),
    //     Default::default(),
    //     "../src/bindings2.ts",
    //     "// My custom header\n".into(),
    // )
    // .unwrap();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            hello_world,
            goodbye_world,
            nested::some_struct
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
