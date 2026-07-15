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
//! cargo add tauri@2.0 specta@=2.0.0-rc.25 specta-typescript@0.0.12
//! cargo add tauri-specta@=2.0.0-rc.25 --features derive,typescript,javascript # `javascript` for JSDoc, `typescript` for Typescript
//! ```
//!
//! ## Features
//!
//! There are the following optional features which can be enabled:
//!
//! - `derive` - Enables the `Event` derive macro. This is only required if your using events.
//! - `javascript` - Enables the JSDoc exporter.
//! - `typescript` - Enables the Typescript exporter.
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
//! ```rust
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
    // TODO: Tauri Specta logo
    html_logo_url = "https://github.com/specta-rs/specta/raw/main/.github/logo-128.png",
    html_favicon_url = "https://github.com/specta-rs/specta/raw/main/.github/logo-128.png"
)]

mod builder;
mod casing;
mod commands;
mod event;
mod lang;
mod macros;
mod name;

pub use builder::{Builder, BuilderConfiguration, ErrorHandlingMode};
pub use casing::Casing;
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

#[doc(hidden)]
pub mod internal {
    //! Internal logic for Tauri Specta.
    //! Nothing in this module has to conform to semver so it should not be used outside of this crate.
    //! It has to be public so the macro's can access it.

    use std::{any::TypeId, sync::Arc};

    use specta::{
        Types,
        datatype::{self, DataType},
    };
    use tauri::{Runtime, ipc::Invoke};

    use super::*;

    /// called by `collect_commands` to construct `Commands`
    pub fn command<R: Runtime, F, T>(f: F, types: T) -> Commands<R>
    where
        F: Fn(Invoke<R>) -> bool + Send + Sync + 'static,
        T: Fn(&mut Types) -> Vec<datatype::Function> + Send + Sync + 'static,
    {
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
