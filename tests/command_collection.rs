//! Regression coverage for the hygiene of `collect_commands!`.

use specta::Types;
use tauri::test::MockRuntime;
use tauri_specta::{Commands, collect_commands};

#[tauri::command]
#[specta::specta]
fn export() {}

#[tauri::command]
#[specta::specta]
fn types(first: String) -> String {
    first
}

#[tauri::command]
#[specta::specta]
fn collected(first: String, second: String) -> String {
    first + &second
}

#[test]
fn collector_does_not_shadow_command_names() {
    let commands: Commands<MockRuntime> = collect_commands![export, types, collected];
    let metadata = (commands.1)(&mut Types::default());
    let signatures: Vec<_> = metadata
        .commands
        .iter()
        .map(|command| (command.name.as_ref(), command.args.len()))
        .collect();

    assert_eq!(signatures, [("export", 0), ("types", 1), ("collected", 2)]);
}
