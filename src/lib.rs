//! Tauri Specta will generate a [Typescript](https://www.typescriptlang.org) or [JSDoc](https://jsdoc.app) file (powered by [Specta](https://docs.rs/specta)) to provide a typesafe interface to your Tauri backend.
//!
//! ## Installation
//!
//! <section class="warning">
//!
//! Tauri Specta v2 is still in beta, and requires using [Specta v2 beta](https://github.com/specta-rs/specta) until it lands as stable.
//!
//! During the beta period, it is really important you use `=` before your Specta version to ensure your project will not break after future updates!
//!
//! </section>
//!
//! To get started run the following commands to add the required dependencies to your `Cargo.toml`:
//!
//! ```sh
//! cargo add tauri@2.0 specta@=2.0.0-rc.28 specta-typescript@0.0.15
//! cargo add tauri-specta@=2.0.0-rc.28 --features derive,typescript,javascript # `javascript` for JSDoc, `typescript` for Typescript
//! ```
//!
//! ## Features
//!
//! There are the following optional features which can be enabled:
//!
//! - `v2` *(enabled by default)* - Pins Tauri to stable v2. Without it, Cargo's
//!   highest-match rule can select the v3 prerelease admitted by the dependency
//!   range.
//! - `v3` - Opts in to Tauri v3 support. Use it with `default-features = false`.
//! - `derive` - Enables the `Event` derive macro. This is only required if your using events.
//! - `javascript` - Enables the JSDoc exporter.
//! - `typescript` - Enables the Typescript exporter.
//!
//! ## Tauri versions
//!
//! Tauri Specta supports Tauri v2 and experimentally supports **Tauri
//! 3.0.0-alpha.6**. The examples below use v2. **Stable Tauri v2 is the default**,
//! including for code-generation-only crates that never depend on `tauri`
//! themselves.
//!
//! ### Selecting a version
//!
//! The `v2` feature is enabled by default and adds a `tauri = "2"` constraint, so
//! Cargo cannot drift an unconstrained graph onto the v3 prerelease admitted by
//! Tauri Specta's own dependency range.
//!
//! To use Tauri v3, make all three changes together:
//!
//! 1. Disable Tauri Specta's default features.
//! 2. Enable the `v3` feature.
//! 3. Add the Tauri v3 dependency yourself.
//!
//! ```toml
//! [dependencies]
//! tauri = "=3.0.0-alpha.6"
//! tauri-runtime-wry = "=3.0.0-alpha.6"
//! tauri-specta = { version = "=2.0.0-rc.29", default-features = false, features = [
//!     "derive",
//!     "typescript",
//!     "v3",
//! ] }
//! # Same treatment if you use it: `default-features = false, features = ["v3"]`.
//! tauri-specta-query = { version = "=2.0.0-rc.29", default-features = false, features = ["v3"] }
//!
//! [build-dependencies]
//! tauri-build = "=3.0.0-alpha.4"
//! ```
//!
//! With neither `v2` nor `v3` enabled, the crate fails to compile rather than
//! silently selecting a version. Enabling `v3` while the graph still resolves
//! stable v2 also fails, because the crate asserts on a v3-only type. A Tauri v2
//! dependency cannot coexist with v3: both declare `links = "Tauri"`, so Cargo
//! refuses to build a graph containing both.
//!
//! ### Using Tauri v3 in an application
//!
//! `build.rs` is unchanged:
//!
//! ```rust,ignore
//! fn main() {
//!     tauri_build::build()
//! }
//! ```
//!
//! Tauri Specta's API is the same, but name the runtime explicitly and select it
//! when building the app. `tauri::Wry` does not exist in v3:
//!
//! ```rust,ignore
//! use specta_typescript::Typescript;
//! use tauri_specta::{Builder, collect_commands, collect_events};
//!
//! #[tauri::command]
//! #[specta::specta]
//! fn hello_world(my_name: String) -> String {
//!     format!("Hello, {my_name}! You've been greeted from Rust!")
//! }
//!
//! fn main() {
//!     // `DynRuntime` is v3's type-erased default. A concrete runtime such as
//!     // `tauri_runtime_wry::WryRuntime` also works when you want static dispatch.
//!     let builder = Builder::<tauri::DynRuntime>::new()
//!         .commands(collect_commands![hello_world]);
//!
//!     #[cfg(debug_assertions)]
//!     builder
//!         .export(Typescript::default(), "../src/bindings.ts")
//!         .expect("Failed to export typescript bindings");
//!
//!     tauri::Builder::default()
//!         // v3 selects the runtime here instead of through the `wry` feature.
//!         .runtime(tauri_runtime_wry::Wry::default())
//!         .invoke_handler(builder.invoke_handler())
//!         .setup(move |app| {
//!             // Still required if you use events.
//!             builder.mount_events(app);
//!             Ok(())
//!         })
//!         .run(tauri::generate_context!())
//!         .expect("error while running tauri application");
//! }
//! ```
//!
//! Generated bindings are unchanged: import `commands`, `events`, and
//! `tauriEvents` from the exported file as on v2. Keep the JavaScript API on the
//! same major as Rust, so `@tauri-apps/api@3.0.0-alpha.3` with alpha.6. For the
//! covered API, generated bindings are byte-identical between the two majors; CI
//! compares them.
//!
//! Other v3 differences:
//!
//! - Rust 1.95 or newer is required. v2 keeps its lower minimum.
//! - `tauri/wry`, `tauri::Wry`, and `tauri::wry` are gone. Use
//!   `tauri::DynRuntime` (type-erased) or a concrete runtime type, and add a
//!   runtime crate such as `tauri-runtime-wry`.
//! - Build with `tauri-build` v3 and use v3-compatible Tauri plugins.
//! - Metadata collection and binding export do not require a webview runtime.
//!
//! ### Prerelease limitations
//!
//! Published v3 alpha.6 does not yet implement Specta support for
//! `tauri::ipc::Request` and `tauri::ipc::Response`; commands using them require
//! an application-level Tauri patch until the upstream support is released.
//! Ordinary commands, events, semantic channel payload transforms, and query
//! exports use the shared implementation on the published frontend APIs.
//!
//! Tauri Specta's range admits earlier 3.0.0 alphas, but only alpha.6 is tested;
//! newer alphas and v3 stable need a tested dependency-bound update. Pin the
//! exact version, as every v3 snippet above does.
//!
//! ## Setup
//!
//! The follow is a minimal example of how to setup Tauri Specta with Typescript.
//!
//! ```rust,no_run
//! #![cfg_attr(
//!     all(not(debug_assertions), target_os = "windows"),
//!     windows_subsystem = "windows"
//! )]
//!
//! use serde::{Deserialize, Serialize};
//! use specta_typescript::Typescript;
//! use tauri_specta::{collect_commands, Builder};
//!
//! #[tauri::command]
//! #[specta::specta] // < You must annotate your commands
//! fn hello_world(my_name: String) -> String {
//!     format!("Hello, {my_name}! You've been greeted from Rust!")
//! }
//!
//! fn main() {
//!     let mut builder = Builder::new()
//!         // Then register them (separated by a comma)
//!         .commands(collect_commands![hello_world,]);
//!
//!     #[cfg(debug_assertions)] // <- Only export on non-release builds
//!     builder
//!         .export(Typescript::default(), "../src/bindings.ts")
//!         .expect("Failed to export typescript bindings");
//!
//!     tauri::Builder::default()
//!         // and finally tell Tauri how to invoke them
//!         .invoke_handler(builder.invoke_handler())
//!         .setup(move |app| {
//!             // This is also required if you want to use events
//!             builder.mount_events(app);
//!
//!             Ok(())
//!         })
//!         .run(tauri::test::mock_context(tauri::test::noop_assets()))
//!         .expect("error while running tauri application");
//! }
//! ```
//!
//! ## Export to JSDoc
//!
//! If your interested in using JSDoc instead of Typescript you can replace the [`specta_typescript::Typescript`](https://docs.rs/specta-typescript/latest/specta_typescript/struct.Typescript.html) struct
//! with [`specta_jsdoc::JSDoc`](https://docs.rs/specta-jsdoc/latest/specta_jsdoc/struct.JSDoc.html) like the following:
//!
//! ```rust,no_run
//! use specta_typescript::JSDoc;
//!
//! let mut builder = tauri_specta::Builder::<tauri::Wry>::new();
//!
//! #[cfg(debug_assertions)]
//! builder
//!     .export(JSDoc::default(), "../src/bindings.js")
//!     .expect("Failed to export jsdoc bindings");
//! ```
//!
//! ## Usage on frontend
//!
//! ```typescript
//! import { commands, events } from "./bindings"; // This should point to the file we export from Rust
//!
//! console.log(await commands.greet("Brendan"));
//! ```
//!
//! ## Error handling
//!
//! By default a failed command rejects the promise the binding returns, matching a plain
//! `invoke`. Command results are typed as the command's success type: for
//! `fn has_error() -> Result<String, i32>`, `commands.hasError()` returns `Promise<string>`.
//!
//! [`Builder::error_handling`] changes how a `Result`-returning command is exposed:
//!
//! - [`ErrorHandlingMode::Throw`] (the default) — the promise rejects.
//! - [`ErrorHandlingMode::Result`] — resolves to `{ status: "ok"; data: T }` or
//!   `{ status: "error"; error: E }`.
//! - [`ErrorHandlingMode::ResultUnknown`] — as `Result`, but the error is typed `unknown`
//!   and the injected runtime never rethrows.
//! - [`ErrorHandlingMode::DataError`] — resolves to `{ data: T; error: null }` or
//!   `{ data: null; error: E }`.
//!
//! A rejected `invoke` can carry either the command's own error value or a framework-level
//! failure (an unknown command, a badly typed argument, an ACL denial), and the two are
//! indistinguishable at the JavaScript boundary. That is why `Result` rethrows anything
//! that is an `Error` instance and why `ResultUnknown` types the error as `unknown`.
//! [`Builder::typed_error_impl`] replaces the injected runtime entirely.
//!
//! ## Channels
//!
//! A [`tauri::ipc::Channel`] streams values from a command to the frontend. A
//! channel is only valid as a direct argument or return value of a command, and
//! the generated binding types it as `Channel<T>`.
//!
//! ```rust,no_run
//! use tauri::ipc::Channel;
//!
//! #[tauri::command]
//! #[specta::specta]
//! fn download(url: String, on_progress: Channel<u32>) {
//!     for chunk in [10, 20, 100] {
//!         let _ = on_progress.send(chunk);
//!     }
//! }
//! ```
//!
//! ```ts
//! import { Channel } from "@tauri-apps/api/core";
//! import { commands } from "./bindings";
//!
//! const onProgress = new Channel<number>();
//! onProgress.onmessage = (progress) => console.log(progress);
//!
//! await commands.download("https://example.com/file.zip", onProgress);
//! ```
//!
//! A channel cannot be nested inside a struct, enum, or event payload. Only a
//! top-level command argument or return value is turned into a channel, so a
//! channel in a nested position is invalid. To send a channel alongside other
//! data, put a [`tauri::ipc::JavaScriptChannelId`] in the payload and call
//! `channel_on` to get a `Channel`:
//!
//! ```rust,ignore
//! use tauri::ipc::{Channel, JavaScriptChannelId};
//!
//! // `JavaScriptChannelId` has no `specta::Type` impl, so export it as a string.
//! #[derive(serde::Deserialize, specta::Type)]
//! struct DownloadRequest {
//!     url: String,
//!     #[specta(type = String)]
//!     on_progress: JavaScriptChannelId,
//! }
//!
//! #[tauri::command]
//! #[specta::specta]
//! fn download(request: DownloadRequest, webview: tauri::Webview<tauri::Wry>) {
//!     let on_progress: Channel<u32> = request.on_progress.channel_on(webview);
//!     // ...
//! }
//! ```
//!
//! When a channel's payload contains a [semantic type](#semantic-frontend-types),
//! the generated binding registers the payload transform on the channel using
//! the published `onmessage` getter/setter API. The channel id Tauri addresses stays
//! unchanged. Handlers can be replaced after registration, and repeated registration
//! replaces rather than stacks transforms. A channel can only carry one deserializer.
//! The mapping remains attached even when the channel is passed to
//! a command whose payload has **no transform**. Use a fresh channel when switching
//! between transformed and untransformed payloads, and do not share a channel
//! concurrently between commands needing different transforms. Registration adds
//! properties to the channel, so leave it extensible (do not freeze or seal it).
//!
//! ## Custom types
//!
//! Similar to [`serde::Serialize`] you must put the [`specta::Type`] derive macro on your own types to allow Specta to understand your types. For example:
//! ```rust
//! use serde::{Serialize, Deserialize};
//! use specta::Type;
//!
//! #[derive(Serialize, Deserialize, Type)]
//! pub struct MyStruct {
//!     a: String
//! }
//!
//! // Call `typ()` as much as you want.
//! let mut builder = tauri_specta::Builder::<tauri::Wry>::new().typ::<MyStruct>();
//! ```
//!
//! ## Events
//!
//! You can also make events typesafe by following the following example:
//!
//! ```rust,no_run
//! use serde::{Serialize, Deserialize};
//! use specta::Type;
//! use tauri_specta::{Builder, collect_commands, collect_events, Event};
//!
//! // Add `tauri_specta::Event` to your event
//! #[derive(Serialize, Deserialize, Debug, Clone, Type, Event)]
//! pub struct DemoEvent(String);
//!
//! let mut builder = Builder::new()
//!         // and then register it to your builder
//!         .events(collect_events![DemoEvent]);
//!
//! tauri::Builder::default()
//!         .invoke_handler(builder.invoke_handler())
//!         .setup(move |app| {
//!             // Ensure you mount your events!
//!             builder.mount_events(app);
//!
//!             // Now you can use them
//!
//!             DemoEvent::listen(app, |event| {
//!                 println!("{:?}", event.payload);
//!             });
//!
//!             DemoEvent("Test".into()).emit(app).unwrap();
//!
//!             Ok(())
//!         });
//! ```
//!
//! and it can be used on the frontend like the following:
//!
//! ```ts
//! import { commands, events } from "./bindings";
//! import { appWindow } from "@tauri-apps/api/window";
//!
//! // For all windows
//! events.demoEvent.listen((e) => console.log(e));
//!
//! // For a single window
//! events.demoEvent(appWindow).listen((e) => console.log(e));
//!
//! // Emit to the backend and all windows
//! await events.demoEvent.emit("Test")
//!
//! // Emit to a window
//! await events.demoEvent(appWindow).emit("Test")
//! ```
//!
//! Refer to [`Event`] for all the possible methods for listening and emitting events.
//!
//! ### Tauri's built-in events
//!
//! Tauri emits its own events for window and webview lifecycle changes and for file
//! drag-and-drop (`tauri://resize`, `tauri://drag-drop`, …). They aren't registered
//! through [`Builder::events`], but unlike [`Builder::mocks`] or
//! [`Builder::semantic_types`] there is nothing to opt into: Tauri sends them either way,
//! so the bindings always include a `tauriEvents` object with one typed accessor per
//! built-in event, alongside your own `events`:
//!
//! ```ts
//! import { tauriEvents } from "./bindings";
//!
//! tauriEvents.windowResized.listen((e) => {
//!     console.log(e.payload.width, e.payload.height);
//! });
//!
//! tauriEvents.windowDestroyed.listen(() => console.log("window closed"));
//! ```
//!
//! Event names and payloads mirror Tauri 2.12, and an event Tauri adds later won't
//! appear until Tauri Specta is updated.
//!
//! ## Testing
//!
//! [`Builder::mocks`] exports a typed mock helper alongside your bindings, so a frontend
//! test can stub commands without hand-writing a module that satisfies the bindings'
//! types:
//!
//! ```rust
//! # use tauri_specta::Builder;
//! let builder = Builder::<tauri::Wry>::new().mocks();
//! ```
//!
//! The bindings then export `MockCommandHandlers`, which is one handler per command
//! accessor, and `mockCommands`, which installs them through
//! [`@tauri-apps/api/mocks`](https://v2.tauri.app/develop/tests/mocking/)'s `mockIPC`.
//! This needs `@tauri-apps/api` v2.7 or newer.
//!
//! Because the mock sits at the IPC boundary, the application under test keeps importing
//! the real `commands` and `events` objects, and everything around the call — the
//! configured [`ErrorHandlingMode`], channel payload mapping, event plumbing — behaves as
//! it does in the app:
//!
//! ```ts
//! import { afterEach, expect, test } from "vitest";
//! import { clearMocks } from "@tauri-apps/api/mocks";
//! import { commands, mockCommands } from "./bindings";
//!
//! afterEach(() => clearMocks());
//!
//! test("greet", async () => {
//!     const calls = mockCommands({ greet: ({ myName }) => `Hello, ${myName}!` });
//!
//!     await expect(commands.greet("Brendan")).resolves.toBe("Hello, Brendan!");
//!     expect(calls.calls).toEqual([
//!         { command: "greet", args: { myName: "Brendan" } },
//!     ]);
//!
//!     calls.clear(); // the handlers stay installed
//! });
//! ```
//!
//! A handler receives the command's arguments as a single object and returns the
//! command's success value; throw, or return a rejected promise, to fail the command.
//! A command with no handler rejects rather than resolving `undefined`, so a test can't
//! pass by accident.
//!
//! `mockCommands` returns a `MockCommandsHandle`: `calls` is the live history of every
//! invocation its handlers observed, oldest first, so it keeps growing for as long as
//! those handlers stay installed. Each entry carries the `commands` accessor that was
//! called and the arguments `invoke` received — not a copy, and not narrowed to an object,
//! since a raw `invoke` may pass Tauri's array or buffer payload shapes. A call is
//! recorded before it is dispatched, so one that fails — an unregistered handler, a
//! handler that throws or rejects — is still in the history. `clear()` empties it without
//! uninstalling anything. A command the bindings don't generate has no accessor, so it is
//! recorded under its raw Tauri command name. Calling `mockCommands` again installs fresh
//! handlers and returns a fresh handle with an empty history, and the previous handle
//! stops growing.
//!
//! Once mocks are enabled, events need no extra setup. Tauri's own event mocking is
//! enabled as part of the install, so `events.demoEvent.listen` and
//! `tauriEvents.windowResized.listen` keep working under test, and the event plugin's
//! internal invokes never reach the recorded history. Tauri's mock does not implement
//! `emitTo`, which resolves without reaching any listener. `clearMocks` from
//! `@tauri-apps/api/mocks` uninstalls the handlers but leaves a handle's history alone.
//!
//! Mocks and [`Builder::semantic_types`] don't mix: where a semantic rule applies a
//! transform, the value a handler sees is not the type the bindings declare, and the
//! transform's output type can't be recovered to type the handler. Rather than emit a
//! binding that lies, the export fails and says which command is responsible.
//!
//! ## Phase-specific types
//!
//! By default, Tauri Specta exports types using Serde-aware serialize and deserialize phases. When a Rust type has different Serde shapes for serialization and deserialization, Tauri Specta emits separate TypeScript aliases for those phases:
//!
//! ```ts
//! export type MyType_Serialize = ...;
//! export type MyType_Deserialize = ...;
//! export type MyType = MyType_Serialize;
//! ```
//!
//! The normal alias represents values serialized from Rust. Tauri Specta uses
//! the explicit deserialize shape for command arguments and the serialize shape
//! for command results. If the shapes are identical, only the normal type alias
//! may be emitted.
//!
//! This allows proper type narrowing on Serde attributes which aren't uniformly applied like `rename(serialize = ..., deserialize = ...)`, `skip_serializing`, or `skip_deserializing`.
//!
//! Refer to [`specta_serde::PhasesFormat`] for more information.
//!
//! If you do not want separate serialize/deserialize shapes, you can disable it via [`Builder::disable_serde_phases`].
//!
//! ## Semantic frontend types
//!
//! Tauri Specta supports enabling [Semantic Types](specta_typescript::semantic).
//! This enables you to support richer types which have a non-JSON runtime shape for example [`Date`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date), [`Uint8Array`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Uint8Array), or [`URL`](https://developer.mozilla.org/en-US/docs/Web/API/URL).
//!
//! To enable this feature pass in a configuration, like the following:
//! ```rs
//! use specta_typescript::semantic;
//!
//! tauri_specta::Builder::new()
//!     // This will enable the built-in rules. Refer to the `semantic` module docs for more information.
//!     .semantic_types(semantic::Types::default())
//!     // ...
//! #;
//! ```
//!
//! Checkout the [`semantic`](specta_typescript::semantic) module docs for more information.
//!
//! ## BigInt handling
//!
//! By default Specta Typescript forbids exporting large integer types as they will be truncated by the webview.
//!
//! [Checkout](specta_typescript::Error#bigint-forbidden) the official Specta guidance on your options to resolve this issue.
//!
//! If you would like to opt-in to the dangerous behavior of truncating your integers,
//! you can use [`Builder::dangerously_cast_bigints_to_number`] but we do not recommend it!
//!
//! ## Naming convention (casing)
//!
//! By default Tauri Specta renames your Rust `snake_case` commands and events to
//! JavaScript-idiomatic `camelCase` in the generated bindings (e.g. `hello_world`
//! becomes `commands.helloWorld`).
//!
//! You can override this with [`Builder::function_casing`]. See [`Casing`] for the supported
//! conventions.
//!
//! Command accessor names and command argument names are separate concerns:
//!
//! - Function casing changes only the generated frontend property, such as
//!   `commands.someName` to `commands.some_name`. It does not change the Rust
//!   command name or the command string sent in the runtime IPC message.
//! - Argument renaming changes the IPC payload contract and is configured per
//!   command. Keep `#[tauri::command(rename_all = "...")]` and
//!   `#[specta(rename_all = "...")]` aligned when renaming arguments.
//!
//! [`Builder::function_casing`] does not rename command arguments. See the
//! [command-versus-argument explanation](https://github.com/specta-rs/tauri-specta/issues/164#issuecomment-4976416006)
//! for more context.
//!
//! ```rust
//! use tauri_specta::{Builder, Casing};
//!
//! let mut builder = Builder::<tauri::Wry>::new()
//!     // Keep the original Rust naming for command and event accessors.
//!     .function_casing(Casing::SnakeCase);
//! ```
//!
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://github.com/specta-rs/tauri-specta/raw/main/.github/logo.png",
    html_favicon_url = "https://github.com/specta-rs/tauri-specta/raw/main/.github/logo.png"
)]

#[cfg(not(any(feature = "v2", feature = "v3")))]
compile_error!(
    "`tauri-specta` needs a Tauri major. Enable the default `v2` feature, or disable default \
     features and enable `v3` together with a Tauri v3 dependency. This build has neither."
);

// `tauri::DynRuntime` was introduced in v3 and does not exist in v2. A `v3` opt-in
// whose graph still resolves stable v2 (for example a leftover `tauri = "2"`) fails
// here instead of silently building against the wrong major. All-features builds
// enable `v2` too, which keeps this off.
#[cfg(all(feature = "v3", not(feature = "v2")))]
const _: () = {
    const fn assert_tauri_v3<R: tauri::Runtime>() {}
    assert_tauri_v3::<tauri::DynRuntime>();
};

mod builder;
mod casing;
mod commands;
mod event;
mod lang;
mod macros;
mod name;

pub use builder::{Builder, BuilderConfiguration, ErrorHandlingMode};
pub use casing::Casing;
#[doc(hidden)]
pub use commands::CollectedCommands;
pub use commands::Commands;
pub use event::{Event, Events, TypedEvent};
pub use lang::LanguageExt;

/// Implements the [`Event`](trait@crate::Event) trait for a struct.
///
/// Refer to the [`Event`](trait@crate::Event) trait for more information.
///
#[cfg(feature = "derive")]
#[cfg_attr(docsrs, doc(cfg(feature = "derive")))]
pub use tauri_specta_macros::Event;

pub(crate) use event::EventRegistry;

// The pin crate has no API. Its value is the `tauri = "2"` dependency edge it puts
// into the graph, so it is intentionally never named by this crate's code.
#[cfg(feature = "v2")]
use tauri_specta_v2_pin as _;
#[doc(hidden)]
pub mod internal {
    //! Internal logic for Tauri Specta.
    //! Nothing in this module has to conform to semver so it should not be used outside of this crate.
    //! It has to be public so the macro's can access it.

    use std::{any::TypeId, collections::BTreeMap, sync::Arc};

    use specta::{
        Types,
        datatype::{self, DataType},
    };
    use tauri::{Runtime, ipc::Invoke};

    use crate::commands::{CollectedCommands, apply_command_renames};

    use super::*;

    /// Re-exported so `collect_commands!` can build the `__tauri_command_name_*` idents
    /// without requiring users to depend on `paste` themselves.
    #[doc(hidden)]
    pub use paste::paste as _tauri_specta_paste;

    /// called by `collect_commands` to construct `Commands`
    pub fn command<R: Runtime, F, T>(
        f: F,
        types: T,
        renames: &[(&'static str, &'static str)],
    ) -> Commands<R>
    where
        F: Fn(Invoke<R>) -> bool + Send + Sync + 'static,
        T: Fn(&mut Types) -> Vec<datatype::Function> + Send + Sync + 'static,
    {
        let renames = renames.to_vec();
        let types = move |registry: &mut Types| {
            let mut collected = CollectedCommands {
                functions: types(registry),
                accessor_names: BTreeMap::new(),
            };
            apply_command_renames(&mut collected, &renames);
            collected
        };

        Commands(Arc::new(f), Arc::new(types))
    }

    /// called by `collect_events` to register events to an `Events`
    #[allow(clippy::panic)]
    pub fn register_event<E: Event>(Events(events): &mut Events) {
        if events
            .insert(E::NAME, |types| {
                (
                    TypeId::of::<E>(),
                    match E::definition(types) {
                        DataType::Reference(r) => r,
                        _ => panic!("Can't register event {} with non-reference type", E::NAME),
                    },
                )
            })
            .is_some()
        {
            panic!("Another event with name {} is already registered!", E::NAME)
        }
    }
}
