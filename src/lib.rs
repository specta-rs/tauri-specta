//! Typesafe Tauri commands
//!
//! ## Install
//!
//! ```bash
//! cargo add specta
//! cargo add tauri-specta --features javascript,typescript
//! ```
//!
//! ## Adding Specta to custom types
//!
//! ```rust
//! use specta::Type;
//! use serde::{Deserialize, Serialize};
//!
//! // The `specta::Type` macro allows us to understand your types
//! // We implement `specta::Type` on primitive types for you.
//! // If you want to use a type from an external crate you may need to enable the feature on Specta.
//! #[derive(Serialize, Type)]
//! pub struct MyCustomReturnType {
//!     pub some_field: String,
//! }
//!
//! #[derive(Deserialize, Type)]
//! pub struct MyCustomArgumentType {
//!     pub foo: String,
//!     pub bar: i32,
//! }
//! ```
//!
//! ## Annotate your Tauri commands with Specta
//!
//! ```rust
//! # //! #[derive(Serialize, Type)]
//! # pub struct MyCustomReturnType {
//! #    pub some_field: String,
//! # }
//! #[tauri::command]
//! #[specta::specta] // <-- This bit here
//! fn greet3() -> MyCustomReturnType {
//!     MyCustomReturnType {
//!         some_field: "Hello World".into(),
//!     }
//! }
//!
//! #[tauri::command]
//! #[specta::specta] // <-- This bit here
//! fn greet(name: String) -> String {
//!   format!("Hello {name}!")
//! }
//! ```
//!
//! ## Export your bindings
//!
//! ```rust
//! #[tauri::command]
//! #[specta::specta]
//! fn greet() {}
//! #[tauri::command]
//! #[specta::specta]
//! fn greet2() {}
//! #[tauri::command]
//! #[specta::specta]
//! fn greet3() {}

//! use tauri_specta::*;
//!
//! // this example exports your types on startup when in debug mode or in a unit test. You can do whatever.
//! fn main() {
//!     #[cfg(debug_assertions)]
//!		ts::builder()
//!			.commands(collect_commands![greet, greet2, greet3])
//!			.path("../src/bindings.ts")
//!			.export()
//!			.unwrap();
//!
//!     // or export to JS with JSDoc
//!     #[cfg(debug_assertions)]
//!		js::builder()
//!			.commands(collect_commands![greet, greet2, greet3])
//!			.path("../src/bindings.js")
//!			.export()
//!			.unwrap();
//! }
//!
//! #[test]
//! fn export_bindings() {
//!		ts::builder()
//!			.commands(collect_commands![greet, greet2, greet3])
//!			.path("../src/bindings.ts")
//!			.export()
//!			.unwrap();
//!
//!		js::builder()
//!			.commands(collect_commands![greet, greet2, greet3])
//!			.path("../src/bindings.js")
//!			.export()
//!			.unwrap();
//! }
//! ```
//!
//! ## Usage on frontend
//!
//! ```ts
//! import { commands } from "./bindings"; // This should point to the file we export from Rust
//!
//! await commands.greet("Brendan");
//! ```
//!
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    // TODO: Tauri Specta logo
    html_logo_url = "https://github.com/oscartbeaumont/specta/raw/main/.github/logo-128.png",
    html_favicon_url = "https://github.com/oscartbeaumont/specta/raw/main/.github/logo-128.png"
)]

use std::{borrow::Cow, error, fmt, path::PathBuf};

use specta::{datatype, TypeMap};

#[cfg(feature = "derive")]
#[cfg_attr(docsrs, doc(cfg(feature = "derive")))]
pub use tauri_specta_macros::Event;

mod builder;
#[doc(hidden)]
pub mod internal;
mod macros;

pub use builder::Builder; // TODO: Rename

/// The exporter for [Javascript](https://www.javascript.com).
#[cfg(feature = "javascript")]
#[cfg_attr(docsrs, doc(cfg(feature = "javascript")))]
pub mod js;

/// The exporter for [TypeScript](https://www.typescriptlang.org).
#[cfg(feature = "typescript")]
#[cfg_attr(docsrs, doc(cfg(feature = "typescript")))]
pub mod ts;

#[cfg(any(feature = "javascript", feature = "typescript"))]
mod js_ts;

mod event;
mod statics;

// TODO: Probs drop
pub use event::*;
pub use statics::StaticCollection;

/// A set of functions that produce language-specific code
pub trait ExportLanguage: 'static {
    type Config: Default + Clone; // TODO: Remove
    type Error: fmt::Debug + error::Error + From<std::io::Error>; // TODO: Review if this `From<std::io::Error>` should be removed before stabilisation

    fn run_format(path: PathBuf, cfg: &ExportConfig<Self::Config>);

    fn render_events(
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig<Self::Config>,
    ) -> Result<String, Self::Error>;

    /// Renders a collection of [`FunctionDataType`] into a string.
    fn render_commands(
        commands: &[datatype::Function],
        type_map: &TypeMap,
        cfg: &ExportConfig<Self::Config>,
    ) -> Result<String, Self::Error>;

    /// Renders the output of [`globals`], [`render_functions`] and all dependant types into a TypeScript string.
    fn render(
        commands: &[datatype::Function],
        events: &[EventDataType],
        type_map: &TypeMap,
        statics: &StaticCollection,
        cfg: &ExportConfig<Self::Config>,
    ) -> Result<String, Self::Error>;
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PluginName(&'static str);

pub(crate) enum ItemType {
    Event,
    Command,
}

impl PluginName {
    pub fn new(plugin_name: &'static str) -> Self {
        Self(plugin_name)
    }

    pub fn apply_as_prefix(&self, s: &str, item_type: ItemType) -> String {
        format!(
            "plugin:{}{}{}",
            self.0,
            match item_type {
                ItemType::Event => ":",
                ItemType::Command => "|",
            },
            s,
        )
    }
}

/// The configuration for the generator
#[derive(Default, Clone)]
pub struct ExportConfig<TConfig> {
    /// The name of the plugin to invoke.
    ///
    /// If there is no plugin name (i.e. this is an app), this should be `None`.
    pub(crate) plugin_name: Option<PluginName>,
    /// The specta export configuration
    pub(crate) inner: TConfig,
    pub(crate) path: Option<PathBuf>,
    pub(crate) header: Cow<'static, str>,
}

impl<TConfig: Default> ExportConfig<TConfig> {
    /// Creates a new [`ExportConfiguration`] from a [`specta::ts::ExportConfiguration`]
    pub fn new(config: TConfig) -> Self {
        Self {
            inner: config,
            ..Default::default()
        }
    }
}
