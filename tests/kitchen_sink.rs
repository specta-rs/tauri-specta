//! Kitchen sink snapshot tests.
//!
//! Exports a builder covering every supported feature through each exporter,
//! layout, and builder configuration, then compares the generated bindings
//! against the committed fixtures in `tests/snapshots/`.
//!
//! Run `UPDATE_SNAPSHOTS=1 cargo test --all-features` to regenerate fixtures
//! after an intentional output change, then review and commit the diff. The
//! TypeScript fixtures are also type-checked by `pnpm run check:bindings`
//! (`tests/typescript/tsconfig.json`) so an invalid binding can't be blessed
//! into a fixture.
//!
//! https://github.com/specta-rs/tauri-specta/issues/197
//! https://github.com/specta-rs/tauri-specta/issues/208

#![allow(missing_docs, unused, deprecated, clippy::unwrap_used, clippy::panic)]
#![cfg(all(feature = "typescript", feature = "javascript", feature = "derive"))]

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::{JSDoc, Layout, Typescript};
use tauri::test::MockRuntime;
use tauri_specta::{
    Builder, Casing, ErrorHandlingMode, Event, LanguageExt, collect_commands, collect_events,
};

// ----- types -----

/// A struct with a doc comment and common field shapes.
#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct Profile {
    user_id: u32,
    display_name: String,
    tags: Vec<String>,
    homepage: Option<String>,
    scores: HashMap<String, i32>,
}

#[derive(Serialize, Deserialize, Type)]
struct Point(f64, f64);

#[derive(Serialize, Deserialize, Type)]
struct Wrapper(String);

#[derive(Serialize, Deserialize, Type)]
#[serde(tag = "type", content = "data")]
enum TaggedEnum {
    Unit,
    Tuple(String),
    Struct { id: u32 },
}

#[derive(Serialize, Deserialize, Type)]
#[serde(untagged)]
enum UntaggedEnum {
    Number(i32),
    Text(String),
}

#[derive(Serialize, Deserialize, Type)]
struct PhaseSpecificRename {
    #[serde(rename(serialize = "serialized_value", deserialize = "deserialized_value"))]
    value: String,
}

#[derive(Serialize, Deserialize, Type)]
struct BigNumbers {
    unsigned: u64,
    signed: i64,
}

#[derive(Debug, Serialize, Type)]
enum PlainError {
    NotFound,
    PermissionDenied(String),
}

#[derive(Debug, Serialize, Type)]
#[serde(tag = "type", content = "data")]
enum TaggedError {
    Io(String),
    Parse { line: u32 },
}

/// Registered via `Builder::typ` without being referenced by a command.
#[derive(Serialize, Deserialize, Type)]
struct ManuallyRegistered {
    name: String,
}

// ----- events -----

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
struct EmptyEvent;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
struct MessageEvent(String);

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
struct StructEvent {
    event_id: u32,
    payload: String,
}

// ----- commands -----

/// A command with a doc comment.
#[tauri::command]
#[specta::specta]
fn greet(my_name: String) -> String {
    format!("Hello, {my_name}!")
}

#[tauri::command]
#[specta::specta]
fn unit_command() {}

#[tauri::command(async)]
#[specta::specta]
async fn async_command(delay_ms: u32) -> u32 {
    delay_ms
}

#[tauri::command]
#[specta::specta]
fn many_args(a: String, b: i32, c: bool, d: Vec<String>, e: Option<String>) -> bool {
    c
}

#[tauri::command]
#[specta::specta]
fn echo_profile(profile: Profile) -> Profile {
    profile
}

#[tauri::command]
#[specta::specta]
fn tuple_types(point: Point) -> Wrapper {
    Wrapper(format!("{}:{}", point.0, point.1))
}

#[tauri::command]
#[specta::specta]
fn enums(tagged: TaggedEnum, untagged: UntaggedEnum) -> TaggedEnum {
    tagged
}

#[tauri::command]
#[specta::specta]
fn phase_specific_rename(input: PhaseSpecificRename) -> PhaseSpecificRename {
    input
}

#[tauri::command]
#[specta::specta]
fn big_numbers(input: BigNumbers) -> BigNumbers {
    input
}

#[tauri::command]
#[specta::specta]
fn plain_error() -> Result<String, PlainError> {
    Err(PlainError::NotFound)
}

#[tauri::command]
#[specta::specta]
fn tagged_error() -> Result<Profile, TaggedError> {
    Err(TaggedError::Parse { line: 7 })
}

#[tauri::command]
#[specta::specta]
fn nullable_result() -> Result<Option<String>, PlainError> {
    Ok(None)
}

#[tauri::command]
#[specta::specta]
fn maybe() -> Option<String> {
    None
}

#[tauri::command]
#[specta::specta]
fn with_channel(channel: tauri::ipc::Channel<i32>) {}

#[tauri::command]
#[specta::specta]
fn skipped_args(state: tauri::State<'_, String>, visible: String) -> String {
    visible
}

#[tauri::command]
#[specta::specta]
fn generic<R: tauri::Runtime>(window: tauri::Window<R>) {}

#[deprecated = "This is a deprecated command"]
#[tauri::command]
#[specta::specta]
fn deprecated_command() {}

// ----- builders -----

fn kitchen_sink() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new()
        .commands(collect_commands![
            greet,
            unit_command,
            async_command,
            many_args,
            echo_profile,
            tuple_types,
            enums,
            phase_specific_rename,
            plain_error,
            tagged_error,
            nullable_result,
            maybe,
            with_channel,
            skipped_args,
            generic::<MockRuntime>,
            deprecated_command,
        ])
        .events(collect_events![EmptyEvent, MessageEvent, StructEvent])
        .typ::<ManuallyRegistered>()
        .constant("APP_NAME", "Kitchen Sink")
        .constant("MAX_RETRIES", 3)
        .constant("DEBUG_DEFAULT", false)
}

fn commands_only() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new().commands(collect_commands![greet, unit_command, plain_error])
}

fn events_only() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new().events(collect_events![EmptyEvent, StructEvent])
}

fn constants_only() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new()
        .constant("APP_NAME", "Kitchen Sink")
        .constant("MAX_RETRIES", 3)
}

fn empty() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new()
}

/// Libraries that a generated `mockCommands` handler map collides with: `toString` and
/// `valueOf` are `Object.prototype` members, so a lookup that doesn't check own
/// properties finds an inherited function instead of reporting the command as unmocked.
#[tauri::command]
#[specta::specta]
fn to_string() -> String {
    "to string".to_string()
}

#[tauri::command]
#[specta::specta]
fn value_of() -> String {
    "value of".to_string()
}

fn prototype_keys() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new().commands(collect_commands![to_string, value_of])
}

/// A mocks scenario in a wrapping error handling mode, so the generated handlers are also
/// exercised against `typedError`.
fn mocks_result() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new()
        .commands(collect_commands![greet, plain_error, nullable_result])
        .error_handling(ErrorHandlingMode::Result)
}

/// `Casing::KebabCase` produces accessors like `unit-command`, which are not valid
/// identifiers and have to be quoted wherever the mocks name them.
fn kebab() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new()
        .commands(collect_commands![greet, unit_command])
        .function_casing(Casing::KebabCase)
}

// `u64`/`i64` fail the default BigInt export behavior, so these commands only
// appear in the `dangerously_cast_bigints_to_number` scenarios.
fn bigints() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new()
        .commands(collect_commands![big_numbers])
        .dangerously_cast_bigints_to_number()
}

// Unified mode rejects asymmetric serde renames, so this is the kitchen sink
// without `phase_specific_rename` (see `no_serde_phases_rejects_asymmetric_renames`).
fn unified() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new()
        .commands(collect_commands![
            greet,
            unit_command,
            async_command,
            many_args,
            echo_profile,
            tuple_types,
            enums,
            plain_error,
            tagged_error,
            nullable_result,
            maybe,
            with_channel,
            skipped_args,
            generic::<MockRuntime>,
            deprecated_command,
        ])
        .events(collect_events![EmptyEvent, MessageEvent, StructEvent])
        .typ::<ManuallyRegistered>()
        .constant("APP_NAME", "Kitchen Sink")
        .constant("MAX_RETRIES", 3)
        .constant("DEBUG_DEFAULT", false)
        .disable_serde_phases()
}

// ----- snapshot harness -----

/// Every scenario the suite exports, one snapshot directory each. Kept in sync
/// with the tests below by `scenario_list_matches_snapshots`, so renaming or
/// deleting a test can't leave dead fixtures behind (or silently drop coverage).
const SCENARIOS: &[&str] = &[
    "bigints_as_number_jsdoc",
    "bigints_as_number_ts",
    "commands_only_jsdoc",
    "commands_only_ts",
    "constants_only_jsdoc",
    "constants_only_ts",
    "empty_jsdoc",
    "empty_ts",
    "error_data_error_jsdoc",
    "error_data_error_ts",
    "error_result_jsdoc",
    "error_result_ts",
    "error_result_unknown_jsdoc",
    "error_result_unknown_ts",
    "error_throw_jsdoc",
    "error_throw_ts",
    "events_only_jsdoc",
    "events_only_mocks_jsdoc",
    "events_only_mocks_ts",
    "events_only_ts",
    "kitchen_sink_files_jsdoc",
    "kitchen_sink_files_ts",
    "kitchen_sink_jsdoc",
    "kitchen_sink_mocks_jsdoc",
    "kitchen_sink_mocks_ts",
    "kitchen_sink_namespaces_ts",
    "kitchen_sink_ts",
    "mocks_kebab_jsdoc",
    "mocks_kebab_ts",
    "mocks_prototype_keys_ts",
    "mocks_result_ts",
    "no_serde_phases_jsdoc",
    "no_serde_phases_ts",
    "plugin_jsdoc",
    "plugin_ts",
];

/// `UPDATE_SNAPSHOTS=1` (or `true`) regenerates the committed fixtures. Any other
/// value — including `0` — leaves them untouched, so a stray export in a shell
/// can't silently bless a diff.
fn updating_snapshots() -> bool {
    matches!(
        std::env::var("UPDATE_SNAPSHOTS").as_deref(),
        Ok("1") | Ok("true")
    )
}

fn scratch_dir(scenario: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tauri_specta_kitchen_sink_{}_{scenario}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn snapshot_root(scenario: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(scenario)
}

fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path.strip_prefix(root).unwrap().to_path_buf());
            }
        }
    }
    files.sort();
    files
}

fn copy_tree(from: &Path, to: &Path) {
    for file in collect_files(from) {
        let dest = to.join(&file);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(from.join(&file), dest).unwrap();
    }
}

fn assert_matches_snapshot(scenario: &str, produced_root: &Path) {
    let snapshot = snapshot_root(scenario);

    if updating_snapshots() {
        if snapshot.exists() {
            fs::remove_dir_all(&snapshot).unwrap();
        }
        copy_tree(produced_root, &snapshot);
        return;
    }

    assert!(
        snapshot.exists(),
        "no snapshot for `{scenario}`. Run `UPDATE_SNAPSHOTS=1 cargo test --all-features` and commit the result"
    );

    let expected_files = collect_files(&snapshot);
    let produced_files = collect_files(produced_root);
    assert_eq!(
        expected_files, produced_files,
        "generated file set for `{scenario}` differs from the snapshot. Run `UPDATE_SNAPSHOTS=1 cargo test --all-features` if this is intentional"
    );

    for file in expected_files {
        let expected = fs::read_to_string(snapshot.join(&file)).unwrap();
        let produced = fs::read_to_string(produced_root.join(&file)).unwrap();
        if expected != produced {
            let line = expected
                .lines()
                .zip(produced.lines())
                .position(|(e, p)| e != p)
                .map(|i| i + 1)
                .unwrap_or_else(|| expected.lines().count().min(produced.lines().count()) + 1);
            panic!(
                "generated output for `{scenario}` differs from snapshot `{}` at line {line}.\n\
                 expected: {:?}\n\
                 produced: {:?}\n\
                 Run `UPDATE_SNAPSHOTS=1 cargo test --all-features` if this change is intentional",
                file.display(),
                expected.lines().nth(line - 1).unwrap_or("<end of file>"),
                produced.lines().nth(line - 1).unwrap_or("<end of file>"),
            );
        }
    }
}

fn assert_export<L: LanguageExt>(
    scenario: &str,
    file_name: &str,
    builder: &Builder<MockRuntime>,
    language: L,
) {
    assert!(
        SCENARIOS.contains(&scenario),
        "scenario `{scenario}` is missing from SCENARIOS"
    );

    let dir = scratch_dir(scenario);
    builder
        .export(language, dir.join(file_name))
        .expect("failed to export bindings");
    assert_matches_snapshot(scenario, &dir);
}

#[test]
fn scenario_list_matches_snapshots() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots");

    let mut on_disk: Vec<String> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();

    let mut declared: Vec<&str> = SCENARIOS.to_vec();
    declared.sort_unstable();

    assert_eq!(
        on_disk, declared,
        "tests/snapshots and SCENARIOS disagree: every scenario needs exactly one snapshot directory. \
         Run `UPDATE_SNAPSHOTS=1 cargo test --all-features` after adding a test, and delete the directory \
         (and its SCENARIOS entry) when removing one"
    );
}

// ----- default configuration -----

#[test]
fn kitchen_sink_ts() {
    assert_export(
        "kitchen_sink_ts",
        "bindings.ts",
        &kitchen_sink(),
        Typescript::default(),
    );
}

#[test]
fn kitchen_sink_jsdoc() {
    assert_export(
        "kitchen_sink_jsdoc",
        "bindings.js",
        &kitchen_sink(),
        JSDoc::default(),
    );
}

// ----- layouts -----

#[test]
fn kitchen_sink_files_ts() {
    assert_export(
        "kitchen_sink_files_ts",
        "bindings",
        &kitchen_sink(),
        Typescript::default().layout(Layout::Files),
    );
}

#[test]
fn kitchen_sink_files_jsdoc() {
    assert_export(
        "kitchen_sink_files_jsdoc",
        "bindings",
        &kitchen_sink(),
        JSDoc::default().layout(Layout::Files),
    );
}

#[test]
fn kitchen_sink_namespaces_ts() {
    assert_export(
        "kitchen_sink_namespaces_ts",
        "bindings.ts",
        &kitchen_sink(),
        Typescript::default().layout(Layout::Namespaces),
    );
}

// ----- content subsets -----

#[test]
fn commands_only_ts() {
    assert_export(
        "commands_only_ts",
        "bindings.ts",
        &commands_only(),
        Typescript::default(),
    );
}

#[test]
fn commands_only_jsdoc() {
    assert_export(
        "commands_only_jsdoc",
        "bindings.js",
        &commands_only(),
        JSDoc::default(),
    );
}

#[test]
fn events_only_ts() {
    assert_export(
        "events_only_ts",
        "bindings.ts",
        &events_only(),
        Typescript::default(),
    );
}

#[test]
fn events_only_jsdoc() {
    assert_export(
        "events_only_jsdoc",
        "bindings.js",
        &events_only(),
        JSDoc::default(),
    );
}

#[test]
fn constants_only_ts() {
    assert_export(
        "constants_only_ts",
        "bindings.ts",
        &constants_only(),
        Typescript::default(),
    );
}

#[test]
fn constants_only_jsdoc() {
    assert_export(
        "constants_only_jsdoc",
        "bindings.js",
        &constants_only(),
        JSDoc::default(),
    );
}

#[test]
fn empty_ts() {
    assert_export("empty_ts", "bindings.ts", &empty(), Typescript::default());
}

#[test]
fn empty_jsdoc() {
    assert_export("empty_jsdoc", "bindings.js", &empty(), JSDoc::default());
}

// ----- error handling modes -----

#[test]
fn error_throw_ts() {
    let builder = kitchen_sink().error_handling(ErrorHandlingMode::Throw);
    assert_export(
        "error_throw_ts",
        "bindings.ts",
        &builder,
        Typescript::default(),
    );
}

#[test]
fn error_throw_jsdoc() {
    let builder = kitchen_sink().error_handling(ErrorHandlingMode::Throw);
    assert_export(
        "error_throw_jsdoc",
        "bindings.js",
        &builder,
        JSDoc::default(),
    );
}

#[test]
fn error_result_ts() {
    let builder = kitchen_sink().error_handling(ErrorHandlingMode::Result);
    assert_export(
        "error_result_ts",
        "bindings.ts",
        &builder,
        Typescript::default(),
    );
}

#[test]
fn error_result_jsdoc() {
    let builder = kitchen_sink().error_handling(ErrorHandlingMode::Result);
    assert_export(
        "error_result_jsdoc",
        "bindings.js",
        &builder,
        JSDoc::default(),
    );
}

#[test]
fn error_data_error_ts() {
    let builder = kitchen_sink().error_handling(ErrorHandlingMode::DataError);
    assert_export(
        "error_data_error_ts",
        "bindings.ts",
        &builder,
        Typescript::default(),
    );
}

#[test]
fn error_data_error_jsdoc() {
    let builder = kitchen_sink().error_handling(ErrorHandlingMode::DataError);
    assert_export(
        "error_data_error_jsdoc",
        "bindings.js",
        &builder,
        JSDoc::default(),
    );
}

#[test]
fn error_result_unknown_ts() {
    let builder = kitchen_sink().error_handling(ErrorHandlingMode::ResultUnknown);
    assert_export(
        "error_result_unknown_ts",
        "bindings.ts",
        &builder,
        Typescript::default(),
    );
}

#[test]
fn error_result_unknown_jsdoc() {
    let builder = kitchen_sink().error_handling(ErrorHandlingMode::ResultUnknown);
    assert_export(
        "error_result_unknown_jsdoc",
        "bindings.js",
        &builder,
        JSDoc::default(),
    );
}

// ----- builder configuration -----

#[test]
fn no_serde_phases_ts() {
    assert_export(
        "no_serde_phases_ts",
        "bindings.ts",
        &unified(),
        Typescript::default(),
    );
}

#[test]
fn no_serde_phases_jsdoc() {
    assert_export(
        "no_serde_phases_jsdoc",
        "bindings.js",
        &unified(),
        JSDoc::default(),
    );
}

#[test]
fn no_serde_phases_rejects_asymmetric_renames() {
    let builder = kitchen_sink().disable_serde_phases();
    let dir = scratch_dir("no_serde_phases_rejects_asymmetric_renames");
    let err = builder
        .export(Typescript::default(), dir.join("bindings.ts"))
        .expect_err("unified mode should reject asymmetric serde renames")
        .to_string();
    assert!(
        err.contains("PhaseSpecificRename.value"),
        "unexpected error: {err}"
    );
}

#[test]
fn bigints_as_number_ts() {
    assert_export(
        "bigints_as_number_ts",
        "bindings.ts",
        &bigints(),
        Typescript::default(),
    );
}

#[test]
fn bigints_as_number_jsdoc() {
    assert_export(
        "bigints_as_number_jsdoc",
        "bindings.js",
        &bigints(),
        JSDoc::default(),
    );
}

#[test]
fn plugin_ts() {
    let builder = kitchen_sink().plugin_name("kitchen-sink");
    assert_export("plugin_ts", "bindings.ts", &builder, Typescript::default());
}

#[test]
fn plugin_jsdoc() {
    let builder = kitchen_sink().plugin_name("kitchen-sink");
    assert_export("plugin_jsdoc", "bindings.js", &builder, JSDoc::default());
}

// ----- mocks -----

#[test]
fn kitchen_sink_mocks_ts() {
    assert_export(
        "kitchen_sink_mocks_ts",
        "bindings.ts",
        &kitchen_sink().mocks(),
        Typescript::default(),
    );
}

#[test]
fn kitchen_sink_mocks_jsdoc() {
    assert_export(
        "kitchen_sink_mocks_jsdoc",
        "bindings.js",
        &kitchen_sink().mocks(),
        JSDoc::default(),
    );
}

#[test]
fn events_only_mocks_ts() {
    assert_export(
        "events_only_mocks_ts",
        "bindings.ts",
        &events_only().mocks(),
        Typescript::default(),
    );
}

#[test]
fn events_only_mocks_jsdoc() {
    assert_export(
        "events_only_mocks_jsdoc",
        "bindings.js",
        &events_only().mocks(),
        JSDoc::default(),
    );
}

#[test]
fn mocks_kebab_ts() {
    assert_export(
        "mocks_kebab_ts",
        "bindings.ts",
        &kebab().mocks(),
        Typescript::default(),
    );
}

#[test]
fn mocks_kebab_jsdoc() {
    assert_export(
        "mocks_kebab_jsdoc",
        "bindings.js",
        &kebab().mocks(),
        JSDoc::default(),
    );
}

#[test]
fn mocks_prototype_keys_ts() {
    assert_export(
        "mocks_prototype_keys_ts",
        "bindings.ts",
        &prototype_keys().mocks(),
        Typescript::default(),
    );
}

#[test]
fn mocks_result_ts() {
    assert_export(
        "mocks_result_ts",
        "bindings.ts",
        &mocks_result().mocks(),
        Typescript::default(),
    );
}

/// Mocks type each handler against the command's success value, which is the same for
/// every error handling mode: a mock fails a command by throwing, not by returning `Err`.
#[test]
fn mocks_ignore_error_handling_mode() {
    let mocks = |builder: Builder<MockRuntime>| {
        let path = std::env::temp_dir().join(format!(
            "tauri-specta-mocks-mode-{}-{:?}.ts",
            std::process::id(),
            std::thread::current().id()
        ));
        builder
            .mocks()
            .export(Typescript::default(), &path)
            .unwrap();
        let bindings = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_file(&path);
        bindings
            .split_once("/* Tauri Specta mocks */")
            .expect("the bindings should contain a mocks section")
            .1
            .to_owned()
    };

    let result = mocks(kitchen_sink());
    for mode in [
        ErrorHandlingMode::Throw,
        ErrorHandlingMode::ResultUnknown,
        ErrorHandlingMode::DataError,
    ] {
        assert_eq!(
            result,
            mocks(kitchen_sink().error_handling(mode)),
            "the mocks section should not depend on the error handling mode"
        );
    }
}

/// The mocks are emitted into the framework runtime file, which means they have to
/// qualify user types exactly the way `commands` does when the layout splits them out.
#[test]
fn mocks_follow_the_layout() {
    let dir = scratch_dir("mocks_follow_the_layout");
    kitchen_sink()
        .mocks()
        .export(
            Typescript::default().layout(Layout::Files),
            dir.join("bindings"),
        )
        .expect("failed to export bindings");

    let index = fs::read_to_string(dir.join("bindings/index.ts")).unwrap();
    assert!(
        index.contains(r#"import type * as kitchen_sink from "./kitchen_sink";"#),
        "{index}"
    );
    assert!(
        index.contains("(args: { profile: kitchen_sink.Profile_Deserialize })"),
        "mock handler types should be qualified like the commands they stand in for:\n{index}"
    );
    assert!(index.contains("kitchen_sink.Profile_Serialize"), "{index}");
}
