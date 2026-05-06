#![allow(missing_docs, unused)]

use serde::Serialize;
use specta::{Type, specta};
use tauri::{Runtime, State, Window};

// Test different combinations of results

#[tauri::command]
#[specta]
fn basic() -> String {
    "Hello, world!".to_string()
}

#[tauri::command]
#[specta]
fn result() -> Result<String, ()> {
    Ok("Hello, world!".to_string())
}

#[tauri::command(async)]
#[specta]
async fn async_() -> String {
    "Hello, world!".to_string()
}

#[tauri::command(async)]
#[specta]
async fn async_result() -> Result<String, ()> {
    Ok("Hello, world!".to_string())
}

#[tauri::command(async)]
#[specta]
async fn async_impl() -> impl Serialize + Type {
    "Hello, World!"
}

// Test different combinations of args

#[tauri::command]
#[specta]
fn value(input: String) -> String {
    "Hello, world!".to_string()
}

#[tauri::command]
#[specta]
fn state(state: State<String>) -> String {
    "Hello, world!".to_string()
}

#[tauri::command]
#[specta]
fn window<R: Runtime>(window: Window<R>) -> String {
    "Hello, world!".to_string()
}

#[tauri::command]
#[specta]
fn state_value(state: State<()>, a: String) -> String {
    "Hello, world!".to_string()
}

#[tauri::command]
#[specta]
#[allow(clippy::boxed_local)]
fn state_many_values(state: State<()>, a: String, b: i32, c: bool, d: Box<u128>) -> String {
    "Hello, world!".to_string()
}
