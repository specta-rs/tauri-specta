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

const PLUGIN_NAME: &str = "specta-example";

fn builder<R: Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::new()
        .plugin_name(PLUGIN_NAME)
        .commands(collect_commands![add_numbers])
        .events(collect_events![RandomNumber])
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    let builder = builder();

    Builder::new(PLUGIN_NAME)
        .invoke_handler(builder.invoke_handler())
        .setup(move |app, _| {
            builder.mount_events(app);

            let app = app.clone();
            std::thread::spawn(move || loop {
                RandomNumber(rand::random()).emit(&app).unwrap();
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
        builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier),
                "./bindings.ts",
            )
            .expect("failed to export specta types");
    }
}
