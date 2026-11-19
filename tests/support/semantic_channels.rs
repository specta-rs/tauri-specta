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

#[derive(Serialize, Type)]
struct PlainPayload {
    url: String,
}

#[tauri::command]
fn stream_urls(channel: tauri::ipc::Channel<Payload>) {
    let _ = channel;
}

#[tauri::command]
fn plain_urls(channel: tauri::ipc::Channel<PlainPayload>) {
    let _ = channel;
}

pub fn builder() -> Builder<MockRuntime> {
    Builder::new()
        .commands(collect_commands![stream_urls, plain_urls])
        .semantic_types(semantic::Configuration::empty().define::<SemanticUrl>(
            |_| define("URL").into(),
            None,
            Some(semantic::Transform::new(|value| {
                format!("new URL({value})")
            })),
        ))
}
