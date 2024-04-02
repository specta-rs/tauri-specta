use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};
use tauri_specta::*;

/// Adds two numbers, returning the result.
#[tauri::command]
#[specta::specta]
fn add_numbers(a: i32, b: i32) -> i32 {
    a + b
}

#[derive(Clone, serde::Serialize, specta::Type, Event)]
struct RandomNumber(i32);

macro_rules! specta_builder {
    () => {
        ts::builder()
            .commands(collect_commands![add_numbers])
            .events(collect_events![RandomNumber])
    };
}

const PLUGIN_NAME: &str = "custom-plugin";

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    let (invoke_handler, register_events) =
        specta_builder!().build_plugin_utils(PLUGIN_NAME).unwrap();

    Builder::new(PLUGIN_NAME)
        .invoke_handler(invoke_handler)
        .setup(move |app| {
            register_events(app);

            let app = app.clone();
            std::thread::spawn(move || loop {
                RandomNumber(rand::random()).emit_all(&app).unwrap();
                std::thread::sleep(std::time::Duration::from_secs(1));
            });

            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        specta_builder!()
            .path("./bindings.ts")
            .config(specta::ts::ExportConfig::default().formatter(specta::ts::formatter::prettier))
            .export_for_plugin(PLUGIN_NAME)
            .ok();
    }
}
