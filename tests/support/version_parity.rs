//! Identical commands and export configurations compiled separately with Tauri v2/v3.

use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::{JSDoc, Typescript};
use tauri::{Runtime, test::MockRuntime};
use tauri_specta::{Builder, Commands, ErrorHandlingMode, Event, collect_commands, collect_events};
use tauri_specta_query::{CommandSet, TanstackQueryFramework};

#[derive(Clone, Debug, Deserialize, Serialize, Type, Event)]
pub struct Progress {
    pub completed: u32,
}

#[tauri::command(rename = "greet-v3")]
#[specta::specta]
pub fn greet(name: String, state: tauri::State<'_, u32>) -> String {
    format!("Hello, {name}: {}", *state)
}

#[tauri::command]
#[specta::specta]
async fn double(value: u32) -> u32 {
    value * 2
}

#[tauri::command]
#[specta::specta]
fn fail() -> Result<String, String> {
    Err("failed".into())
}

#[tauri::command]
#[specta::specta]
fn stream(on_progress: tauri::ipc::Channel<u32>) {
    let _ = on_progress.send(42);
}

#[tauri::command]
#[specta::specta]
pub fn handle<R: Runtime>(_app: tauri::AppHandle<R>) {}

/// A documented command.
///
/// Its docs must render identically for both collectors.
#[tauri::command]
#[specta::specta]
fn documented(value: u32) -> u32 {
    value
}

#[tauri::command]
#[specta::specta]
#[deprecated(since = "1.0.0", note = "use documented")]
fn deprecated_command() -> String {
    String::new()
}

/// An explicit unit result is not sent to the frontend.
#[tauri::command]
#[specta::specta]
fn explicit_unit() {}

#[allow(deprecated)]
pub fn commands() -> Commands<MockRuntime> {
    collect_commands![
        greet,
        double,
        fail,
        stream,
        handle::<MockRuntime>,
        documented,
        deprecated_command,
        explicit_unit
    ]
}

pub fn builder() -> Builder<MockRuntime> {
    Builder::new()
        .commands(commands())
        .events(collect_events![Progress])
}

pub fn export(dir: &Path) {
    // These callers pass dedicated generated-output directories. Never compare
    // stale cached files if an exporter stops producing one of its outputs.
    if dir.exists() {
        std::fs::remove_dir_all(dir).unwrap();
    }
    std::fs::create_dir_all(dir).unwrap();
    let builder = builder().mocks();
    builder
        .export(Typescript::default(), dir.join("bindings.ts"))
        .unwrap();
    builder
        .export(JSDoc::default(), dir.join("bindings.js"))
        .unwrap();
    super::semantic_channels::builder()
        .export(Typescript::default(), dir.join("semantic.ts"))
        .unwrap();
    super::semantic_channels::builder()
        .export(JSDoc::default(), dir.join("semantic.js"))
        .unwrap();

    for (name, mode) in [
        ("result", ErrorHandlingMode::Result),
        ("result-unknown", ErrorHandlingMode::ResultUnknown),
        ("data-error", ErrorHandlingMode::DataError),
    ] {
        let builder = builder.clone().error_handling(mode);
        builder
            .export(Typescript::default(), dir.join(format!("{name}.ts")))
            .unwrap();
        builder
            .export(JSDoc::default(), dir.join(format!("{name}.js")))
            .unwrap();
    }

    let (query, builder) =
        CommandSet::<MockRuntime>::new(collect_commands![greet], collect_commands![double])
            .build(TanstackQueryFramework::React);
    assert!(query.contains("commands.greet"));
    builder
        .export(Typescript::default().with_raw(query), dir.join("query.ts"))
        .unwrap();
    builder
        .plugin_name("demo")
        .export(Typescript::default(), dir.join("plugin.ts"))
        .unwrap();

    let bindings = std::fs::read_to_string(dir.join("bindings.ts")).unwrap();
    assert!(bindings.contains("\"greet-v3\""));
    assert!(bindings.contains("onProgress: Channel<number>"));
    assert!(!bindings.contains("state:"));
    let bindings = std::fs::read_to_string(dir.join("plugin.ts")).unwrap();
    assert!(bindings.contains("plugin:demo|greet-v3"));
}
