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

use core::fmt;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use specta::{
    datatype::{self, DataType},
    Language, SpectaID, TypeMap,
};

use tauri::{ipc::Invoke, Runtime};
/// Implements the [`Event`](trait@crate::Event) trait for a struct.
///
/// Refer to the [`Event`](trait@crate::Event) trait for more information.
///
#[cfg(feature = "derive")]
#[cfg_attr(docsrs, doc(cfg(feature = "derive")))]
pub use tauri_specta_macros::Event;

mod builder;
mod event;
mod lang;
mod macros;

pub use builder::Builder;
pub(crate) use event::EventRegistry;
pub use event::{Event, TypedEvent};

/// A wrapper around the output of the `collect_commands` macro.
///
/// This acts to seal the implementation details of the macro.
#[derive(Clone)]
pub struct Commands<R: Runtime>(
    // Bounds copied from `tauri::Builder::invoke_handler`
    pub(crate) Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    pub(crate) fn(&mut TypeMap) -> Vec<datatype::Function>,
);

impl<R: Runtime> fmt::Debug for Commands<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Commands").finish()
    }
}

impl<R: Runtime> Default for Commands<R> {
    fn default() -> Self {
        Self(
            Arc::new(tauri::generate_handler![]),
            ::specta::function::collect_functions![],
        )
    }
}

/// A wrapper around the output of the `collect_commands` macro.
///
/// This acts to seal the implementation details of the macro.
#[derive(Default)]
pub struct Events(BTreeMap<&'static str, fn(&mut TypeMap) -> (SpectaID, DataType)>);

/// The context of what needs to be exported. Used when implementing [`LanguageExt`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ExportContext {
    pub plugin_name: Option<&'static str>,
    pub commands: Vec<datatype::Function>,
    pub events: BTreeMap<&'static str, DataType>,
    pub type_map: TypeMap,
    pub constants: HashMap<Cow<'static, str>, serde_json::Value>,
}

/// Implemented for all languages which Tauri Specta supports exporting to.
///
/// Currently implemented for:
///  - [`specta_typescript::Typescript`]
///  - [`specta_jsdoc::JSDoc`]
pub trait LanguageExt: Language {
    fn render_commands(&self, cfg: &ExportContext) -> Result<String, Self::Error>;
    fn render_events(&self, cfg: &ExportContext) -> Result<String, Self::Error>;
    fn render(&self, cfg: &ExportContext) -> Result<String, Self::Error>;
}

impl<L: LanguageExt> LanguageExt for &L {
    fn render_commands(&self, cfg: &ExportContext) -> Result<String, Self::Error> {
        (*self).render_commands(cfg)
    }

    fn render_events(&self, cfg: &ExportContext) -> Result<String, Self::Error> {
        (*self).render_events(cfg)
    }

    fn render(&self, cfg: &ExportContext) -> Result<String, Self::Error> {
        (*self).render(cfg)
    }
}

pub(crate) enum ItemType {
    Event,
    Command,
}

pub(crate) fn apply_as_prefix(plugin_name: &str, s: &str, item_type: ItemType) -> String {
    format!(
        "plugin:{}{}{}",
        plugin_name,
        match item_type {
            ItemType::Event => ":",
            ItemType::Command => "|",
        },
        s,
    )
}

#[doc(hidden)]
pub mod internal {
    //! Internal logic for Tauri Specta.
    //! Nothing in this module has to conform to semver so it should not be used outside of this crate.
    //! It has to be public so macro's can access it.

    use super::*;

    /// called by `collect_commands` to construct `Commands`
    pub fn command<R: Runtime, F>(
        f: F,
        types: fn(&mut TypeMap) -> Vec<datatype::Function>,
    ) -> Commands<R>
    where
        F: Fn(Invoke<R>) -> bool + Send + Sync + 'static,
    {
        Commands(Arc::new(f), types)
    }

    /// called by `collect_events` to register events to an `Events`
    pub fn register_event<E: Event>(Events(events): &mut Events) {
        if events
            .insert(E::NAME, |type_map| {
                (E::sid(), E::reference(type_map, &[]).inner)
            })
            .is_some()
        {
            panic!("Another event with name {} is already registered!", E::NAME)
        }
    }
}
