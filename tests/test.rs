use serde::Serialize;
use specta::{specta, Type};
use tauri::{State, Window};

// Test different combinations of results

#[tauri::command]
#[specta]
fn one() -> String {
    format!("Hello, world!")
}

#[tauri::command]
#[specta]
fn two() -> Result<String, ()> {
    Ok(format!("Hello, world!"))
}

#[tauri::command(async)]
#[specta]
async fn three() -> String {
    format!("Hello, world!")
}

#[tauri::command(async)]
#[specta]
async fn four() -> Result<String, ()> {
    Ok(format!("Hello, world!"))
}

#[tauri::command(async)]
#[specta]
async fn six() -> impl Serialize + Type {
    "Hello, World!"
}

// Test different combinations of args

#[tauri::command]
#[specta]
fn seven(input: String) -> String {
    format!("Hello, world!")
}

#[tauri::command]
#[specta]
fn eight(state: State<String>) -> String {
    format!("Hello, world!")
}

#[tauri::command]
#[specta]
fn nine(window: Window) -> String {
    format!("Hello, world!")
}

#[tauri::command]
#[specta]
fn ten(state: State<()>, a: String) -> String {
    format!("Hello, world!")
}

#[tauri::command]
#[specta]
fn eleven(state: State<()>, a: String, b: i32, c: bool, d: Box<u128>) -> String {
    format!("Hello, world!")
}
