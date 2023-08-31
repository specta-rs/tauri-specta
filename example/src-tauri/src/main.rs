#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::{Deserialize, Serialize};
use specta::{specta, Type};
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

#[tauri::command]
#[specta]
fn has_error() -> Result<&'static str, i32> {
    Err(32)
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

    #[derive(Serialize, Deserialize, Debug, Clone, specta::Type, tauri_specta::Event)]
    pub struct DemoEvent(String);

    #[derive(Serialize, Deserialize, Debug, Clone, specta::Type, tauri_specta::Event)]
    pub struct EmptyEvent;

    tauri::Builder::default()
        .plugin(
            ts::Exporter::new("../src/bindings.ts")
                .with_commands(tauri_specta::collect_commands![
                    hello_world,
                    goodbye_world,
                    nested::some_struct,
                    has_error
                ])
                .with_events(tauri_specta::collect_events![DemoEvent, EmptyEvent])
                .build_plugin(),
        )
        .setup(|app| {
            DemoEvent::listen_global(app.handle(), |event| {
                dbg!(event.payload);
            });

            DemoEvent("Test".to_string()).emit_all(app.handle()).ok();

            let handle = app.handle();

            EmptyEvent::listen_global(handle.clone(), move |_| {
                EmptyEvent.emit_all(handle.clone()).ok();
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
