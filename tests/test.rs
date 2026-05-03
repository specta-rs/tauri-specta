#![allow(unused)]

use serde::Serialize;
use specta::Type;
use tauri::{State, Window};

// Test different combinations of results

#[tauri::command]
fn basic() -> String {
    format!("Hello, world!")
}

#[tauri::command]
fn result() -> Result<String, ()> {
    Ok(format!("Hello, world!"))
}

#[tauri::command(async)]
async fn async_() -> String {
    format!("Hello, world!")
}

#[tauri::command(async)]
async fn async_result() -> Result<String, ()> {
    Ok(format!("Hello, world!"))
}

#[tauri::command(async)]
async fn async_impl() -> impl Serialize + Type {
    "Hello, World!"
}

// Test different combinations of args

#[tauri::command]
fn value(input: String) -> String {
    format!("Hello, world!")
}

#[tauri::command]
fn state(state: State<String>) -> String {
    format!("Hello, world!")
}

#[tauri::command]
fn window<R: tauri::Runtime>(window: Window<R>) -> String {
    format!("Hello, world!")
}

#[tauri::command]
fn state_value(state: State<()>, a: String) -> String {
    format!("Hello, world!")
}

#[tauri::command]
fn state_many_values(state: State<()>, a: String, b: i32, c: bool, d: Box<u128>) -> String {
    format!("Hello, world!")
}
