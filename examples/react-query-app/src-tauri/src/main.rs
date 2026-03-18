#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri::State;
use tauri_specta::{Builder, CommandOutputTarget, collect_commands};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct Todo {
    id: u32,
    title: String,
}

#[derive(Default)]
struct AppState {
    next_id: u32,
    todos: Vec<Todo>,
}

#[derive(Default)]
struct SharedState(std::sync::Mutex<AppState>);

#[tauri::command]
#[specta::specta]
fn greeting(name: String) -> String {
    format!("Hello, {name}! This came from Rust.")
}

#[tauri::command]
#[specta::specta]
fn list_todos(state: State<'_, SharedState>) -> Vec<Todo> {
    state.0.lock().expect("state poisoned").todos.clone()
}

#[tauri::command]
#[specta::specta]
fn create_todo(state: State<'_, SharedState>, title: String) -> Todo {
    let mut state = state.0.lock().expect("state poisoned");
    state.next_id += 1;

    let todo = Todo {
        id: state.next_id,
        title,
    };
    state.todos.push(todo.clone());
    todo
}

#[tauri::command]
#[specta::specta]
fn delete_todo(state: State<'_, SharedState>, id: u32) -> bool {
    let mut state = state.0.lock().expect("state poisoned");
    let original_len = state.todos.len();
    state.todos.retain(|todo| todo.id != id);
    state.todos.len() != original_len
}

fn main() {
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            greeting,
            list_todos,
            create_todo,
            delete_todo
        ])
        .command_output_target(CommandOutputTarget::TanstackQuery)
        .mutation_commands(["create_todo", "delete_todo"]);

    #[cfg(debug_assertions)]
    builder
        .export(Typescript::default(), "../src/bindings.ts")
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .manage(SharedState::default())
        .invoke_handler(builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
