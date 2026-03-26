#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri::{AppHandle, Manager, ipc::Channel};
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
async fn async_hello_world(my_name: String) -> String {
    format!("Hello, {my_name}!")
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

#[tauri::command]
#[specta::specta]
fn with_channel(_channel: tauri::ipc::Channel<i32>) {}

#[derive(Serialize, Deserialize, Type)]
struct PhaseSpecificRename {
    #[serde(rename(serialize = "serialized_value", deserialize = "deserialized_value"))]
    value: String,
}

#[tauri::command]
#[specta::specta]
fn phase_specific_rename(input: PhaseSpecificRename) -> PhaseSpecificRename {
    input
}

#[derive(Debug, Serialize, Deserialize, Type)]
struct SpecialTypes {
    u128_max: u128,
    u128_min: u128,
    i128_max: i128,
    i128_min: i128,
    // bytes: bytes::Bytes,
    // #[specta(type = specta_typescript::Bytes)] // TODO
    // bytes_from_vec: Vec<u8>,
    // TODO: Example with bytes + base64 encoding
    // TODO: Allow `UInt8Array` from any `String`.
    // date: chrono::NaiveDate,
    // datetime: chrono::NaiveDateTime,
}

#[tauri::command]
#[specta::specta]
fn special_types(input: SpecialTypes) -> (SpecialTypes, SpecialTypes) {
    println!("SPECIAL TYPES: {:?}", input);
    println!(
        "ASSERTIONS: {} {} {} {}",
        input.u128_max == u128::MAX,
        input.u128_min == u128::MIN,
        input.i128_max == i128::MAX,
        input.i128_min == i128::MIN
    );

    (
        input,
        SpecialTypes {
            u128_max: u128::MAX,
            u128_min: u128::MIN,
            i128_max: i128::MAX,
            i128_min: i128::MIN,
            // bytes: vec![1, 2, 3, 4].into(),
            // bytes_from_vec: vec![1, 2, 3, 4],
            // date: chrono::Utc::now().date_naive(),
            // datetime: chrono::Utc::now().naive_utc(),
        },
    )
}

fn special_types_w_channel(channel: Channel<u128>) {
    channel.send(u128::MAX).unwrap();
}

fn emit_event_with_bigint(app: AppHandle) {
    EventWithBigInt(u128::MAX).emit(&app).unwrap();
}

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type, tauri_specta::Event)]
pub struct EventWithBigInt(u128);

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
    Err(MyError::IoError(std::io::Error::other("oh no!")))
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

    Err(std::io::Error::other("oh no!").into()) // We use `into` here to do the `From` conversion.
}

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "myDemoEvent")] // Optionally rename event key (for JS/TS)
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
            async_hello_world,
            has_error,
            nested::some_struct,
            generic::<tauri::Wry>,
            deprecated,
            with_channel,
            phase_specific_rename,
            special_types,
            special_types_w_channel,
            emit_event_with_bigint,
            typesafe_errors_using_thiserror,
            typesafe_errors_using_thiserror_with_value,
        ])
        .events(tauri_specta::collect_events![crate::DemoEvent, EmptyEvent])
        .typ::<Custom>()
        .typ::<Testing>()
        .constant("universalConstant", 42);

    #[cfg(debug_assertions)]
    {
        use specta_typescript::{JSDoc, Layout};

        builder
            .export(
                Typescript::default().bigint(specta_typescript::BigIntExportBehavior::Number), // TODO: Remove this
                // .header("/* eslint-disable */")
                "../src/bindings.ts",
            )
            .expect("Failed to export typescript bindings");

        // TODO: Reneable these
        // builder
        //     .export(JSDoc::default(), "../src/bindings-js.js")
        //     .expect("Failed to export typescript bindings");

        // builder
        //     .export(
        //         Typescript::default().layout(Layout::Files),
        //         "../src/bindings-ts-files",
        //     )
        //     .expect("Failed to export typescript bindings");

        // builder
        //     .export(
        //         JSDoc::default().layout(Layout::Files),
        //         "../src/bindings-js-files",
        //     )
        //     .expect("Failed to export typescript bindings");

        // builder
        //     .export(
        //         Typescript::default().layout(Layout::Namespaces),
        //         "../src/bindings-ts-namespaces.ts",
        //     )
        //     .expect("Failed to export typescript bindings");
    }

    #[cfg(debug_assertions)]
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
