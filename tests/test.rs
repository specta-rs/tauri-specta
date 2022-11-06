use serde::Serialize;
use specta::Type;
use tauri::{State, Window};

// Test different combinations of results

#[tauri::command]
#[tauri_specta::command]
fn one() -> String {
    format!("Hello, world!")
}

#[tauri::command]
#[tauri_specta::command]
fn two() -> Result<String, ()> {
    Ok(format!("Hello, world!"))
}

#[tauri::command(async)]
#[tauri_specta::command]
async fn three() -> String {
    format!("Hello, world!")
}

#[tauri::command(async)]
#[tauri_specta::command]
async fn four() -> Result<String, ()> {
    Ok(format!("Hello, world!"))
}

#[tauri::command(async)]
#[tauri_specta::command]
async fn six() -> impl Serialize + Type {
    "Hello, World!"
}

// Test different combinations of args

#[tauri::command]
#[tauri_specta::command]
fn seven(input: String) -> String {
    format!("Hello, world!")
}

#[tauri::command]
#[tauri_specta::command]
fn eight(state: State<String>) -> String {
    format!("Hello, world!")
}

#[tauri::command]
#[tauri_specta::command]
fn nine(window: Window) -> String {
    format!("Hello, world!")
}

#[tauri::command]
#[tauri_specta::command]
fn ten(state: State<()>, a: String) -> String {
    format!("Hello, world!")
}

#[tauri::command]
#[tauri_specta::command]
fn eleven(state: State<()>, a: String, b: i32, c: bool, d: Box<u128>) -> String {
    format!("Hello, world!")
}
