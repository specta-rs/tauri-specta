#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri_specta::*;
use thiserror::Error;

/// HELLO
/// WORLD
/// !!!!
#[tauri::command]
#[specta::specta]
fn hello_world(my_name: String) -> String {
    format!("Hello, {my_name}! You've been greeted from Rust!")
}

#[tauri::command]
#[specta::specta]
fn goodbye_world() -> impl Serialize + specta::Type {
    "Goodbye world :("
}

#[tauri::command]
#[specta::specta]
fn has_error() -> Result<&'static str, i32> {
    Err(32)
}

#[tauri::command]
#[specta::specta]
fn generic<T: tauri::Runtime>(_app: tauri::AppHandle<T>) {}

#[deprecated = "This is a deprecated function"]
#[tauri::command]
#[specta::specta]
fn deprecated() {}

mod nested {
    use super::*;

    #[tauri::command]
    #[specta::specta]
    pub fn some_struct() -> MyStruct {
        MyStruct {
            some_field: "Hello World".into(),
        }
    }

    #[derive(Serialize, specta::Type)] // For Specta support you must add the `specta::Type` derive macro.
    pub struct MyStruct {
        some_field: String,
    }
}

#[derive(Error, Debug, Serialize, Type)]
#[serde(tag = "type", content = "data")]
pub enum MyError {
    // On the frontend this variant will be "IoError" with no data.
    #[error("io error: {0}")]
    IoError(
        #[serde(skip)] // io::Error is not `Serialize` or `Type`
        #[from]
        std::io::Error,
    ),
    // On the frontend this variant will be "AnotherError" with string data.
    #[error("some other error: {0}")]
    AnotherError(String),
}

#[tauri::command]
#[specta::specta]
fn typesafe_errors_using_thiserror() -> Result<(), MyError> {
    Err(MyError::IoError(std::io::Error::new(
        std::io::ErrorKind::Other,
        "oh no!",
    )))
}

#[derive(Error, Debug, Serialize, Type)]
#[serde(tag = "type", content = "data")]
pub enum MyError2 {
    #[error("io error: {0}")]
    IoError(String),
}

impl From<std::io::Error> for MyError2 {
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error.to_string())
    }
}

#[tauri::command]
#[specta::specta]
fn typesafe_errors_using_thiserror_with_value() -> Result<(), MyError2> {
    // some_method()?; // This will work because `?` does `From` conversion.

    Err(std::io::Error::new(std::io::ErrorKind::Other, "oh no!").into()) // We use `into` here to do the `From` conversion.
}

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type, tauri_specta::Event)]
pub struct DemoEvent(String);

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type, tauri_specta::Event)]
pub struct EmptyEvent;

#[derive(Type)]
pub struct Custom(String);

#[derive(Type)]
pub struct Testing {
    a: String,
}

fn main() {
    let builder = Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            hello_world,
            goodbye_world,
            has_error,
            nested::some_struct,
            generic::<tauri::Wry>,
            deprecated,
            typesafe_errors_using_thiserror,
            typesafe_errors_using_thiserror_with_value,
        ])
        .events(tauri_specta::collect_events![crate::DemoEvent, EmptyEvent])
        .typ::<Custom>()
        .constant("universalConstant", 42);

    #[cfg(debug_assertions)]
    builder
        .export(
            Typescript::default()
                .formatter(specta_typescript::formatter::prettier)
                .header("/* eslint-disable */"),
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_jsdoc::JSDoc::default()
                .formatter(specta_typescript::formatter::prettier)
                .header("/* eslint-disable */"),
            "../src/bindings-jsdoc.js",
        )
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            DemoEvent::listen(app, |event| {
                dbg!(event.payload);
            });

            DemoEvent("Test".to_string()).emit(app).ok();

            EmptyEvent::listen(app, |_| {
                println!("Got event from frontend!!");
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
