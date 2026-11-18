//! Shared semantic channel consumer used with both Tauri majors.

use serde::Serialize;
use specta::Type;
use specta_typescript::{define, semantic};
use tauri::test::MockRuntime;
use tauri_specta::{Builder, collect_commands};

#[derive(Serialize, Type)]
struct SemanticUrl(String);

#[derive(Serialize, Type)]
struct Payload {
    url: SemanticUrl,
}

#[tauri::command]
#[specta::specta]
fn stream_urls(channel: tauri::ipc::Channel<Payload>) {
    let _ = channel;
}

pub fn builder() -> Builder<MockRuntime> {
    Builder::new()
        .commands(collect_commands![stream_urls])
        .semantic_types(semantic::Configuration::empty().define::<SemanticUrl>(
            |_| define("URL").into(),
            None,
            Some(semantic::Transform::new(|value| {
                format!("new URL({value})")
            })),
        ))
}
