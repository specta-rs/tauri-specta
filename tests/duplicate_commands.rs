//! Regression coverage for https://github.com/specta-rs/tauri-specta/issues/115.

#![allow(missing_docs, unused, clippy::unwrap_used)]
#![cfg(all(feature = "typescript", feature = "derive"))]

use std::fs;

use specta_typescript::Typescript;
use tauri::test::MockRuntime;
use tauri_specta::{Builder, collect_commands};

#[tauri::command]
#[specta::specta]
fn my_command() {}

#[tauri::command]
#[specta::specta]
fn other_command() {}

// Note: two commands with the same name in different modules of one crate can't
// currently be expressed — specta's generated `__specta__fn__*` macros collide at
// crate root (the same family as issue #142). The runtime check still covers that
// case when it arises across crates (e.g. a plugin command clashing with an app one).

#[allow(non_snake_case)]
#[tauri::command]
#[specta::specta]
fn myCommand() {}

fn export(builder: &Builder<MockRuntime>) -> Result<(), specta_typescript::Error> {
    let dir = std::env::temp_dir().join(format!(
        "tauri_specta_duplicate_commands_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&dir).unwrap();
    builder.export(Typescript::default(), dir.join("bindings.ts"))
}

#[test]
fn distinct_commands_export() {
    let builder =
        Builder::<MockRuntime>::new().commands(collect_commands![my_command, other_command]);
    export(&builder).expect("distinct commands should export");
}

#[test]
fn duplicate_registration_errors() {
    let builder = Builder::<MockRuntime>::new().commands(collect_commands![my_command, my_command]);
    let err = export(&builder)
        .expect_err("registering the same command twice should fail to export")
        .to_string();
    assert!(
        err.contains("Command 'my_command' is registered multiple times"),
        "unexpected error: {err}"
    );
}

#[test]
fn casing_collision_errors() {
    let builder = Builder::<MockRuntime>::new().commands(collect_commands![my_command, myCommand]);
    let err = export(&builder)
        .expect_err("commands that collide after casing should fail to export")
        .to_string();
    assert!(
        err.contains("Commands 'my_command' and 'myCommand' would both export as 'myCommand'"),
        "unexpected error: {err}"
    );
}
