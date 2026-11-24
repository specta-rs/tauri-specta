//! Regression coverage for `tauri::ipc::Channel` placement.
//!
//! Tauri only injects a channel when it is a whole command argument. Nested in a
//! result, event payload, or registered type, the TypeScript exporter used to
//! emit an undefined `TAURI_CHANNEL` reference, so the generated bindings did not
//! type-check. These tests guard that nested channels now fail the export while
//! top-level channel arguments keep working.
//!
//! https://github.com/specta-rs/tauri-specta/issues/198

#![cfg(all(feature = "typescript", feature = "derive"))]
#![allow(missing_docs, dead_code, clippy::unwrap_used, clippy::panic)]

use std::fs;

use serde::Serialize;
use specta::Type;
use specta_typescript::{JSDoc, Typescript, define, semantic};
use tauri::test::MockRuntime;
use tauri_specta::{Builder, Event, LanguageExt, collect_commands, collect_events};

// ----- types -----

#[derive(Serialize, Type)]
struct HasChannel {
    channel: tauri::ipc::Channel<u32>,
}

#[derive(Serialize, Type)]
struct InnerChannel {
    channel: tauri::ipc::Channel<u32>,
}

#[derive(Serialize, Type)]
struct OuterChannel {
    inner: InnerChannel,
}

#[derive(Serialize, Type)]
struct Node {
    value: u32,
    next: Option<Box<Node>>,
}

#[derive(Serialize, Type)]
struct ChannelPayload {
    value: u32,
}

#[derive(Type, Event)]
struct ChannelEvent {
    channel: tauri::ipc::Channel<u32>,
}

#[derive(Serialize, Type)]
struct ManuallyRegisteredChannel {
    channel: tauri::ipc::Channel<u32>,
}

/// A type whose TypeScript runtime value is not its JSON representation, so the
/// bindings have to transform it at runtime.
#[derive(Serialize, Type)]
struct SemanticUrl(String);

#[derive(Serialize, Type)]
struct SemanticPayload {
    url: SemanticUrl,
}

// ----- commands -----

#[tauri::command]
fn channel_in_result() -> HasChannel {
    unimplemented!()
}

#[tauri::command]
fn channel_through_outer() -> OuterChannel {
    unimplemented!()
}

#[tauri::command]
fn recursive_result() -> Node {
    unimplemented!()
}

#[tauri::command]
fn channel_arg(channel: tauri::ipc::Channel<u32>) {
    let _ = channel;
}

#[tauri::command]
fn channel_result() -> tauri::ipc::Channel<u32> {
    unimplemented!()
}

#[tauri::command]
fn channel_in_result_wrapper() -> Result<tauri::ipc::Channel<u32>, String> {
    unimplemented!()
}

#[tauri::command]
fn payload_channel_arg(channel: tauri::ipc::Channel<ChannelPayload>) {
    let _ = channel;
}

#[tauri::command]
fn semantic_channel_arg(channel: tauri::ipc::Channel<SemanticPayload>) {
    let _ = channel;
}

fn semantic_types() -> semantic::Configuration {
    semantic::Configuration::empty().define::<SemanticUrl>(
        |_| define("URL").into(),
        None,
        Some(semantic::Transform::new(|value| {
            format!("new URL({value})")
        })),
    )
}

// ----- harness -----

fn export(builder: &Builder<MockRuntime>, scenario: &str) -> Result<String, String> {
    export_with(builder, Typescript::default(), scenario, "bindings.ts")
}

fn export_with<L: LanguageExt>(
    builder: &Builder<MockRuntime>,
    exporter: L,
    scenario: &str,
    file: &str,
) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!(
        "tauri_specta_channel_validation_{}_{scenario}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(file);

    let result = builder
        .export(exporter, &path)
        .map_err(|err| err.to_string());

    let bindings = result
        .as_ref()
        .ok()
        .map(|_| fs::read_to_string(&path).unwrap());
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&dir);

    result.map(|_| bindings.unwrap())
}

// ----- tests -----

#[test]
fn channel_in_returned_struct_is_rejected() {
    let err = export(
        &Builder::<MockRuntime>::new().commands(collect_commands![channel_in_result]),
        "returned_struct",
    )
    .expect_err("a channel field in a result has no valid TypeScript representation");

    assert!(err.contains("HasChannel"), "unexpected error: {err}");
    assert!(
        err.contains("tauri::ipc::Channel"),
        "unexpected error: {err}"
    );
    // The error has to tell the user what to do instead.
    assert!(
        err.contains("direct command argument")
            && err.contains("JavaScriptChannelId")
            && err.contains("channel_on"),
        "unexpected error: {err}"
    );
}

#[test]
fn channel_in_returned_struct_is_rejected_for_jsdoc_too() {
    let err = export_with(
        &Builder::<MockRuntime>::new().commands(collect_commands![channel_in_result]),
        JSDoc::default(),
        "returned_struct_jsdoc",
        "bindings.js",
    )
    .expect_err("the shared runtime path must reject nested channels for JSDoc as well");

    assert!(err.contains("HasChannel"), "unexpected error: {err}");
}

#[test]
fn channel_in_event_payload_is_rejected() {
    let err = export(
        &Builder::<MockRuntime>::new().events(collect_events![ChannelEvent]),
        "event",
    )
    .expect_err("an event payload cannot carry a channel");

    assert!(err.contains("ChannelEvent"), "unexpected error: {err}");
}

#[test]
fn channel_reached_through_another_type_is_rejected() {
    let err = export(
        &Builder::<MockRuntime>::new().commands(collect_commands![channel_through_outer]),
        "nested_type",
    )
    .expect_err("the channel is reached through `OuterChannel`");

    assert!(err.contains("InnerChannel"), "unexpected error: {err}");
}

#[test]
fn channel_in_manually_registered_type_is_rejected() {
    let err = export(
        &Builder::<MockRuntime>::new().typ::<ManuallyRegisteredChannel>(),
        "registered_type",
    )
    .expect_err("`Builder::typ` exports the type body like any other");

    assert!(
        err.contains("ManuallyRegisteredChannel"),
        "unexpected error: {err}"
    );
}

#[test]
fn recursive_type_without_channel_exports() {
    let bindings = export(
        &Builder::<MockRuntime>::new().commands(collect_commands![recursive_result]),
        "recursive",
    )
    .expect("recursive types are legal and must not loop the channel check");

    assert!(bindings.contains("type Node"), "{bindings}");
}

#[test]
fn top_level_channel_argument_exports() {
    let bindings = export(
        &Builder::<MockRuntime>::new().commands(collect_commands![channel_arg]),
        "top_level_channel",
    )
    .expect("a channel as the whole argument is the supported form");

    assert!(bindings.contains("Channel<number>"), "{bindings}");
}

#[test]
fn top_level_channel_argument_with_struct_payload_exports() {
    let bindings = export(
        &Builder::<MockRuntime>::new().commands(collect_commands![payload_channel_arg]),
        "top_level_payload_channel",
    )
    .expect("a channel whose payload is a plain type is still a top-level argument");

    assert!(bindings.contains("Channel<ChannelPayload>"), "{bindings}");
}

#[test]
fn top_level_channel_argument_stays_untouched_without_a_semantic_payload() {
    let bindings = export(
        &Builder::<MockRuntime>::new().commands(collect_commands![payload_channel_arg]),
        "top_level_payload_channel_plain",
    )
    .expect("a channel whose payload needs no transform is passed as-is");

    assert!(
        !bindings.contains("__TAURI_DESERIALIZE_CHANNEL("),
        "the channel should not be touched when its payload is already the wire shape:\n{bindings}"
    );
}

#[test]
fn channel_as_a_command_result_is_rejected() {
    let err = export(
        &Builder::<MockRuntime>::new().commands(collect_commands![channel_result]),
        "channel_result",
    )
    .expect_err("a returned channel arrives as a string, not a frontend channel");

    assert!(err.contains("channel_result"), "unexpected error: {err}");
    assert!(
        err.contains("can only be a direct command argument"),
        "unexpected error: {err}"
    );
    // The error has to explain why, and what to do instead.
    assert!(
        err.contains("__CHANNEL__") && err.contains("Declare the channel as a command argument"),
        "unexpected error: {err}"
    );
}

#[test]
fn channel_inside_a_result_is_rejected_as_a_result() {
    let err = export(
        &Builder::<MockRuntime>::new().commands(collect_commands![channel_in_result_wrapper]),
        "channel_in_result_wrapper",
    )
    .expect_err("wrapping the channel in a `Result` doesn't make it usable in the frontend");

    assert!(
        err.contains("can only be a direct command argument"),
        "the channel should be rejected as a result rather than told to move to the top level: {err}"
    );
    assert!(
        !err.contains("is nested inside"),
        "moving the channel to the top level is not a fix for a result: {err}"
    );
}

#[test]
fn semantic_channel_payload_is_transformed_before_the_handler() {
    let builder = Builder::<MockRuntime>::new()
        .commands(collect_commands![semantic_channel_arg])
        .semantic_types(semantic_types());

    let expected = "__TAURI_DESERIALIZE_CHANNEL(channel, (v) => ({...v,url:new URL(v.url)}))";

    let bindings = export(&builder, "semantic_channel").expect("semantic channel payload exports");
    assert!(bindings.contains(expected), "{bindings}");
    // The transform is registered on the caller's channel by wrapping its handler, so
    // no replacement channel or frontend API feature is needed.
    assert!(
        bindings.contains("Reflect.deleteProperty(channel, \"onmessage\")"),
        "{bindings}"
    );
    assert!(
        bindings.contains("Object.isExtensible(channel)"),
        "{bindings}"
    );
    assert!(!bindings.contains(".deserialize("), "{bindings}");
    assert!(!bindings.contains("mapChannel"), "{bindings}");
    assert!(!bindings.contains("new Channel("), "{bindings}");

    let jsdoc = export_with(
        &builder,
        JSDoc::default(),
        "semantic_channel_jsdoc",
        "bindings.js",
    )
    .expect("semantic channel payload exports for JSDoc");
    assert!(jsdoc.contains(expected), "{jsdoc}");
    assert!(
        jsdoc.contains("Reflect.deleteProperty(channel, \"onmessage\")"),
        "{jsdoc}"
    );
    assert!(jsdoc.contains("Object.isExtensible(channel)"), "{jsdoc}");
    assert!(!jsdoc.contains(".deserialize("), "{jsdoc}");
    assert!(!jsdoc.contains("mapChannel"), "{jsdoc}");
    assert!(!jsdoc.contains("new Channel("), "{jsdoc}");
}
