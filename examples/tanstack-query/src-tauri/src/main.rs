#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri_specta::*;
use thiserror::Error;

// -- Types --

/// A user of the application.
#[derive(Serialize, Deserialize, Type, Clone)]
pub struct User {
    id: u32,
    name: String,
    email: String,
}

#[derive(Serialize, Deserialize, Type, Clone)]
/// A todo item associated with a user.
pub struct Todo {
    id: u32,
    title: String,
    completed: bool,
    user_id: u32,
}

#[derive(Serialize, Deserialize, Type, Default)]
pub struct AppState {
    users: Vec<User>,
    todos: Vec<Todo>,
}

pub type AppStateMutex = Mutex<AppState>;

/// A simple API error type for demonstration purposes.
#[derive(Error, Debug, Serialize, Type)]
#[serde(tag = "type", content = "data")]
pub enum ApiError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("internal error: {0}")]
    Internal(String),
}

// -- Queries (read operations) --

/// Get a user by ID.
///
/// Returns an error if the user does not exist.
#[tauri::command]
#[specta::specta]
fn get_user(state: tauri::State<AppStateMutex>, id: u32) -> Result<User, ApiError> {
    let state = state.lock().unwrap();
    state
        .users
        .iter()
        .find(|user| user.id == id)
        .cloned()
        .ok_or(ApiError::NotFound(format!("User {id} not found")))
}

/// List all users.
#[tauri::command]
#[specta::specta]
fn list_users(state: tauri::State<AppStateMutex>) -> Vec<User> {
    let state = state.lock().unwrap();
    state.users.clone()
}

/// List todos for a specific user, optionally filtering by title.
///
/// If `title` is provided, only todos containing that substring are returned.
#[tauri::command]
#[specta::specta]
fn list_todos(
    state: tauri::State<AppStateMutex>,
    user_id: u32,
    title: Option<String>,
) -> Vec<Todo> {
    let state = state.lock().unwrap();
    state
        .todos
        .iter()
        .filter(|todo| {
            todo.user_id == user_id && title.as_ref().map_or(true, |t| todo.title.contains(t))
        })
        .cloned()
        .collect()
}

// -- Mutations (write operations) --

#[tauri::command]
#[specta::specta]
fn create_user(
    state: tauri::State<AppStateMutex>,
    name: String,
    email: String,
) -> Result<User, ApiError> {
    let mut state = state.lock().unwrap();
    let user = User {
        id: state.users.len() as u32 + 1,
        name,
        email,
    };
    state.users.push(user.clone());
    Ok(user)
}

#[tauri::command]
#[specta::specta]
fn create_todo(
    state: tauri::State<AppStateMutex>,
    title: String,
    user_id: u32,
) -> Result<Todo, ApiError> {
    let mut state = state.lock().unwrap();
    let todo = Todo {
        id: state.todos.len() as u32 + 1,
        title,
        completed: false,
        user_id,
    };
    state.todos.push(todo.clone());
    Ok(todo)
}

/// Delete a user by ID, returning an error if the user does not exist.
#[tauri::command]
#[specta::specta]
fn delete_user(state: tauri::State<AppStateMutex>, id: u32) -> Result<(), ApiError> {
    let mut state = state.lock().unwrap();
    if let Some(pos) = state.users.iter().position(|user| user.id == id) {
        state.users.remove(pos);
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("User {id} not found")))
    }
}

/// Delete a todo by ID, returning an error if the todo does not exist.
#[tauri::command]
#[specta::specta]
fn delete_todo(state: tauri::State<AppStateMutex>, id: u32) -> Result<(), ApiError> {
    let mut state = state.lock().unwrap();
    if let Some(pos) = state.todos.iter().position(|todo| todo.id == id) {
        state.todos.remove(pos);
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("Todo {id} not found")))
    }
}

fn main() {
    let builder = Builder::<tauri::Wry>::new()
        .queries(tauri_specta::collect_commands![
            get_user, list_users, list_todos,
        ])
        .mutations(tauri_specta::collect_commands![
            create_user,
            create_todo,
            delete_user,
            delete_todo,
        ])
        // Set the TanStack Query framework
        .tanstack(TanstackFramework::React);

    #[cfg(debug_assertions)]
    builder
        .export(Typescript::default(), "../src/bindings.ts")
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .manage(AppStateMutex::default())
        .invoke_handler(builder.invoke_handler())
        .setup(move |_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
