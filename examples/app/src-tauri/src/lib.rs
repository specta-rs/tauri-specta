//! Example Tauri application for Tauri Specta.
//!
//! The commands, events, and the Specta builder live in this library so the binary stays a
//! thin entry point, and so the bindings can be regenerated from a test:
//!
//! ```sh
//! cargo test -p tauri-specta-example-app
//! ```

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri_specta::{Builder, Event};
use thiserror::Error;

/// Emitted by [`greet`]. Exported to the frontend as `events.greeted`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct Greeted {
    pub name: String,
}

/// A command takes arguments and returns anything implementing [`specta::Type`].
#[tauri::command]
#[specta::specta]
fn greet(app: tauri::AppHandle, name: String) -> String {
    Greeted { name: name.clone() }.emit(&app).ok();

    format!("Hello, {name}! You've been greeted from Rust!")
}

/// The error returned by [`greet_strictly`], exported alongside the command.
#[derive(Debug, Error, Serialize, Type)]
#[serde(tag = "type", content = "data")]
pub enum GreetError {
    #[error("a name is required")]
    EmptyName,
    #[error("could not read the name: {0}")]
    Read(
        // `std::io::Error` is neither `Serialize` nor `Type`, so it is skipped and this
        // variant reaches the frontend without data.
        #[serde(skip)]
        #[from]
        std::io::Error,
    ),
}

#[tauri::command]
#[specta::specta]
fn greet_strictly(name: String) -> Result<String, GreetError> {
    if name.trim().is_empty() {
        return Err(GreetError::EmptyName);
    }

    Ok(format!("Hello, {name}!"))
}

/// A command can stream values back to the frontend with a `Channel`.
#[tauri::command]
#[specta::specta]
fn count_to(number: u32, channel: tauri::ipc::Channel<u32>) {
    for current in 1..=number {
        channel.send(current).ok();
    }
}

/// Builds the Tauri Specta builder for this example.
pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        // We recommend returning errors as a value rather than throwing, so a failed
        // command resolves to `{ status: "error"; error: E }` and the frontend can
        // handle every case exhaustively. The default just rejects the promise.
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
        .commands(tauri_specta::collect_commands![
            greet,
            greet_strictly,
            count_to,
        ])
        .events(tauri_specta::collect_events![Greeted])
}

/// Exports the TypeScript bindings into `dir`.
pub fn export_bindings(builder: &Builder<tauri::Wry>, dir: &std::path::Path) {
    builder
        .export(Typescript::default(), dir.join("bindings.ts"))
        .expect("failed to export the TypeScript bindings");
}

/// Runs the example application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = builder();

    #[cfg(debug_assertions)] // Only export the bindings on development builds.
    export_bindings(&builder, std::path::Path::new("../src"));

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            Greeted::listen(app, |event| {
                dbg!(event.payload);
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_bindings_to_disk() {
        export_bindings(&builder(), std::path::Path::new("../src"));
    }
}
