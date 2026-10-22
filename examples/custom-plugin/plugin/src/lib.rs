//! Example Tauri plugin built with Tauri Specta.
//!
//! The plugin owns its commands and events, so it exports its own bindings rather than
//! sharing the application's. `index.ts` re-exports them for the frontend.

use std::time::Duration;

use tauri::{
    Runtime,
    plugin::{Builder as PluginBuilder, TauriPlugin},
};
use tauri_specta::{Builder, Event, collect_commands, collect_events};

/// Adds two numbers, returning the result.
#[tauri::command]
#[specta::specta]
fn add_numbers(a: i32, b: i32) -> i32 {
    a + b
}

/// Emitted by the plugin every second. Exported to the frontend as `events.randomNumber`.
#[derive(Debug, Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
struct RandomNumber(i32);

const PLUGIN_NAME: &str = "specta-example";

/// Builds the Tauri Specta builder for the plugin.
fn builder<R: Runtime>() -> Builder<R> {
    Builder::new()
        .plugin_name(PLUGIN_NAME)
        // We recommend returning errors as a value rather than throwing, so a failed
        // command resolves to `{ status: "error"; error: E }` and the frontend can
        // handle every case exhaustively. The default just rejects the promise.
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
        .commands(collect_commands![add_numbers])
        .events(collect_events![RandomNumber])
}

/// Initialises the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    let builder = builder();

    PluginBuilder::new(PLUGIN_NAME)
        .invoke_handler(builder.invoke_handler())
        .setup(move |app, _| {
            builder.mount_events(app);

            let app = app.clone();
            std::thread::spawn(move || {
                loop {
                    RandomNumber(rand::random()).emit(&app).ok();
                    std::thread::sleep(Duration::from_secs(1));
                }
            });

            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_bindings_to_disk() {
        builder::<tauri::Wry>()
            .export(specta_typescript::Typescript::default(), "./bindings.ts")
            .expect("failed to export the TypeScript bindings");
    }
}
