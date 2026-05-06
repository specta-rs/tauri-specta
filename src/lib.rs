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
//! Tauri Specta exports bindings from the point of view of the frontend. Command
//! arguments and emitted event payloads are values that JavaScript sends to Rust,
//! while command results and received event payloads are values that Rust sends to
//! JavaScript. These two directions can have different serialized shapes when a
//! type uses phase-specific Serde attributes such as
//! `#[serde(rename(serialize = "...", deserialize = "..."))]`.
//!
//! By default, exported TypeScript and JSDoc bindings preserve those differences
//! using Specta's phase-aware Serde formatting. Command inputs are rendered using
//! the deserialize phase because Rust deserializes them from JavaScript, command
//! outputs are rendered using the serialize phase because Rust serializes them to
//! JavaScript, and events apply the appropriate phase for `emit` and `listen`.
//! This keeps the generated frontend API aligned with the actual wire format used
//! by Tauri and Serde.
//!
//! If you do not want separate serialize/deserialize shapes, call
//! [`Builder::disable_serde_phases`]. That switches export back to unified Serde
//! formatting, which is simpler but cannot represent phase-specific Serde behavior
//! accurately.
//!
//! ## Semantic frontend types
//!
//! Some Rust types have a JSON-compatible transport shape but a richer JavaScript
//! runtime shape. For example, dates usually cross the IPC boundary as strings,
//! byte buffers as arrays, and URLs as strings, but frontend code often wants to
//! work with `Date`, `Uint8Array`, or `URL` values directly.
//!
//! [`Builder::semantic_types`] enables Specta TypeScript's semantic type system for
//! generated TypeScript and JSDoc bindings. When configured, Tauri Specta uses the
//! semantic rules to remap exported types and to inject runtime transforms around
//! command arguments, command results, event payloads, and channel payloads. The
//! default [`specta_typescript::semantic::Configuration`] handles common ecosystem
//! types such as `chrono` dates, `jiff` dates, `bytes` buffers, and `url::Url`, and
//! custom rules can be added with Specta TypeScript's semantic APIs.
//!
//! See the [`specta_typescript::semantic`] module documentation for the full list
//! of built-in semantic rules and for custom rule examples.
//!
//! ## BigInt handling
//!
//! Specta TypeScript forbids exporting BigInt-style Rust numeric types such as
//! `usize`, `isize`, `i64`, `u64`, `i128`, `u128`, and `f128` as TypeScript
//! `number` by default. This is intentional: values outside JavaScript's safe
//! integer range can lose precision when they are parsed or represented as
//! `number`.
//!
//! Prefer modelling these values losslessly, such as using a smaller integer type
//! when the range is known to be safe, or serializing large integers as strings and
//! converting them to `bigint` on the frontend. Specta's upstream BigInt error
//! documentation describes the tradeoffs and migration paths in more detail:
//! <https://docs.rs/specta-typescript/latest/specta_typescript/struct.Error.html#bigint-forbidden>.
//!
//! [`Builder::dangerously_cast_bigints_to_number`] is an escape hatch for cases
//! where you explicitly accept precision loss. It remaps BigInt-style primitive
//! types, and `specta_typescript::BigInt`, to TypeScript `number` during export.
//! This is crate-wide for the builder and should be treated like an unsafe policy
//! decision: it can make generated bindings compile, but it does not make large
//! numeric values safe to round-trip through JavaScript.
//!
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    // TODO: Tauri Specta logo
    html_logo_url = "https://github.com/specta-rs/specta/raw/main/.github/logo-128.png",
    html_favicon_url = "https://github.com/specta-rs/specta/raw/main/.github/logo-128.png"
)]

mod builder;
mod commands;
mod event;
mod lang;
mod macros;
mod name;

pub use builder::{Builder, BuilderConfiguration, ErrorHandlingMode};
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
    pub fn command<R: Runtime, F>(
        f: F,
        types: fn(&mut Types) -> Vec<datatype::Function>,
    ) -> Commands<R>
    where
        F: Fn(Invoke<R>) -> bool + Send + Sync + 'static,
    {
        Commands(Arc::new(f), types)
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
