//! Commands collected through Tauri's IPC introspection (`#[tauri::command]` only).

#![cfg(all(feature = "v2", feature = "typescript", feature = "derive"))]
#![allow(missing_docs, dead_code, clippy::unwrap_used)]

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri::test::MockRuntime;
use tauri_specta::{Builder, Casing, collect_commands};

#[derive(Serialize, Deserialize, Type)]
struct Payload {
    value: u32,
}

/// A documented command.
///
/// Second line.
#[tauri::command]
fn documented(input: Payload) -> Payload {
    input
}

#[tauri::command]
#[deprecated(since = "1.2.3", note = "use documented instead")]
fn old_command() -> String {
    String::new()
}

/// Injected arguments must not shift the names of the arguments that follow them.
#[tauri::command]
fn injected_first(state: tauri::State<'_, u32>, name: String) -> String {
    format!("{}:{}", *state, name)
}

#[tauri::command]
async fn async_command(value: u32) -> Result<u32, String> {
    Ok(value)
}

#[tauri::command(rename = "renamed_command")]
fn rust_named() -> String {
    String::new()
}

#[tauri::command(rename_all = "snake_case")]
fn renamed_arguments(my_value: u32, another: String) -> u32 {
    let _ = another;
    my_value
}

#[tauri::command]
fn no_arguments() {}

#[tauri::command]
fn channel(on_progress: tauri::ipc::Channel<u32>) {
    let _ = on_progress.send(1);
}

#[tauri::command]
fn opaque() -> impl Serialize + Type {
    "opaque"
}

fn export(builder: &Builder<MockRuntime>, scenario: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "tauri_specta_introspection_{}_{scenario}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bindings.ts");
    builder.export(Typescript::default(), &path).unwrap();
    let bindings = fs::read_to_string(&path).unwrap();
    let _ = fs::remove_dir_all(&dir);
    bindings
}

fn all() -> Builder<MockRuntime> {
    Builder::new().commands(collect_commands![
        documented,
        old_command,
        injected_first,
        async_command,
        rust_named,
        renamed_arguments,
        no_arguments,
        channel,
        opaque
    ])
}

#[test]
fn commands_need_only_the_tauri_attribute() {
    let bindings = export(&all(), "all");

    // The whole point: no `#[specta::specta]` anywhere above, yet every command is typed.
    assert!(
        bindings.contains("documented: (input: Payload, options?: InvokeOptions) => __TAURI_INVOKE<Payload>(\"documented\", { input }, options)"),
        "{bindings}"
    );
    assert!(
        bindings.contains("asyncCommand: (value: number"),
        "{bindings}"
    );
    assert!(bindings.contains("opaque: ("), "{bindings}");
    assert!(bindings.contains("noArguments: ("), "{bindings}");
    assert!(bindings.contains("Channel<number>"), "{bindings}");
}

#[test]
fn rust_docs_and_deprecation_are_exported() {
    let bindings = export(&all(), "docs");

    assert!(bindings.contains("A documented command."), "{bindings}");
    assert!(bindings.contains("Second line."), "{bindings}");
    assert!(bindings.contains("@deprecated"), "{bindings}");
    assert!(bindings.contains("use documented instead"), "{bindings}");
}

#[test]
fn injected_arguments_do_not_shift_argument_names() {
    let bindings = export(&all(), "injected");

    // `state` is injected by Tauri and must be dropped without consuming `name`.
    assert!(
        bindings.contains("injectedFirst: (name: string, options?: InvokeOptions)"),
        "{bindings}"
    );
    assert!(!bindings.contains("state: "), "{bindings}");
}

#[test]
fn renames_keep_the_accessor_and_rename_the_ipc_name() {
    let bindings = export(&all(), "rename");

    assert!(
        bindings.contains(
            "rustNamed: (options?: InvokeOptions) => __TAURI_INVOKE<string>(\"renamed_command\""
        ),
        "{bindings}"
    );
    // `rename_all` is applied by Tauri's metadata, which specta's macro never supported.
    assert!(
        bindings.contains("renamedArguments: (myValue: number, another: string"),
        "{bindings}"
    );
}

#[test]
fn function_casing_applies_to_inferred_accessors() {
    let bindings = export(
        &Builder::new()
            .commands(collect_commands![documented])
            .function_casing(Casing::SnakeCase),
        "casing",
    );

    assert!(
        bindings.contains("documented: (input: Payload"),
        "{bindings}"
    );
}

#[test]
fn plugin_prefix_applies_to_inferred_commands() {
    let bindings = export(
        &Builder::new()
            .commands(collect_commands![documented])
            .plugin_name("example"),
        "plugin",
    );

    assert!(
        bindings.contains("__TAURI_INVOKE<Payload>(\"plugin:example|documented\""),
        "{bindings}"
    );
}

#[test]
fn parity_fixture_still_exports() {
    // The shared fixture keeps `#[specta::specta]` for v3, so Tauri's introspection and
    // the legacy collector must agree on the same commands.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/introspection-parity");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![documented, async_command, no_arguments])
        .events(tauri_specta::collect_events![]);
    builder
        .export(Typescript::default(), dir.join("bindings.ts"))
        .unwrap();

    let bindings = fs::read_to_string(dir.join("bindings.ts")).unwrap();
    assert!(bindings.contains("documented"), "{bindings}");
    let _ = fs::remove_dir_all(&dir);
}
