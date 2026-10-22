//! Example application hosting the [`tauri_plugin_specta_example`] plugin.
//!
//! The plugin owns its commands and events; the application only registers it and then
//! consumes the bindings the plugin exports.

/// Runs the example application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_specta_example::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
