#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::Serialize;
use specta::{specta, Type};
use tauri_specta::{collate_types, export_to_js, export_to_ts};

#[tauri::command]
#[specta]
fn hello_world(my_name: String) -> String {
    format!("Hello, {my_name}! You've been greeted from Rust!")
}

#[tauri::command]
#[specta]
fn goodbye_world() -> impl Serialize + Type {
    format!("Goodbye world :(")
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

    export_to_ts(
        collate_types![hello_world, goodbye_world, nested::some_struct],
        "../src/bindings.ts",
    )
    .unwrap();

    export_to_js(
        collate_types![hello_world, goodbye_world, nested::some_struct],
        "../src/bindings.js",
    )
    .unwrap();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            hello_world,
            goodbye_world,
            nested::some_struct
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
