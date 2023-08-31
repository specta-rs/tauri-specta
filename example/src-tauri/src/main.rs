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

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type, tauri_specta::Event)]
pub struct DemoEvent(String);

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type, tauri_specta::Event)]
pub struct EmptyEvent;

fn main() {
    tauri::Builder::default()
        .plugin(
            ts::Exporter::new("../src/bindings.ts")
                .with_commands(tauri_specta::collect_commands![
                    hello_world,
                    goodbye_world,
                    has_error,
                    nested::some_struct
                ])
                .with_events(tauri_specta::collect_events![DemoEvent, EmptyEvent])
                .build_plugin(),
        )
        .setup(|app| {
            let handle = app.handle();

            DemoEvent::listen_global(&handle, |event| {
                dbg!(event.payload);
            });

            DemoEvent("Test".to_string()).emit_all(&handle).ok();

            EmptyEvent::listen_global(&handle, {
                let handle = handle.clone();
                move |_| {
                    EmptyEvent.emit_all(&handle).ok();
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
