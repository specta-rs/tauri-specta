// #![allow(unused)]

// use serde::Serialize;
// use specta::Type;
// use tauri::{State, Window};
// use tauri_specta::collect_commands;

// // Test different combinations of results

// #[tauri::command]
// fn basic() -> String {
//     format!("Hello, world!")
// }

// #[tauri::command]
// fn result() -> Result<String, ()> {
//     Ok(format!("Hello, world!"))
// }

// #[tauri::command(async)]
// async fn async_() -> String {
//     format!("Hello, world!")
// }

// #[tauri::command(async)]
// async fn async_result() -> Result<String, ()> {
//     Ok(format!("Hello, world!"))
// }

// #[tauri::command(async)]
// async fn async_impl() -> impl Serialize + Type {
//     "Hello, World!"
// }

// // Test different combinations of args

// #[tauri::command]
// fn value(input: String) -> String {
//     format!("Hello, world!")
// }

// #[tauri::command]
// fn state(state: State<String>) -> String {
//     format!("Hello, world!")
// }

// #[tauri::command]
// fn window(window: Window) -> String {
//     format!("Hello, world!")
// }

// #[tauri::command]
// fn state_value(state: State<()>, a: String) -> String {
//     format!("Hello, world!")
// }

// #[tauri::command]
// fn state_many_values(state: State<()>, a: String, b: i32, c: bool, d: Box<u128>) -> String {
//     format!("Hello, world!")
// }

// #[test]
// fn test_collect_commands() {
//     // collect_commands![];
//     // collect_commands![hello_world];
//     // collect_commands![hello_world,];
//     // collect_commands![hello_world, goodbye_world];
//     // collect_commands![generic::<tauri::Wry>];
//     // collect_commands![generic::<tauri::Wry>,];
// }
