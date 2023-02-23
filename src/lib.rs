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
//! # #[specta::specta]
//! # fn greet() {}
//! # #[specta::specta]
//! # fn greet2() {}
//! # #[specta::specta]
//! # fn greet3() {}
//! use specta::collect_types;
//! use tauri_specta::{ts, js};
//!
//! // this example exports your types on startup when in debug mode or in a unit test. You can do whatever.
//! fn main() {
//!     #[cfg(debug_assertions)]
//!     ts::export(collect_types![greet, greet2, greet3], "../src/bindings.ts").unwrap();
//!
//!     // or export to JS with JSDoc
//!     #[cfg(debug_assertions)]
//!     js::export(collect_types![greet, greet2, greet3], "../src/bindings.js").unwrap();
//! }
//!
//! #[test]
//! fn export_bindings() {
//!     ts::export(collect_types![greet, greet2, greet3], "../src/bindings.ts").unwrap();
//!     js::export(collect_types![greet, greet2, greet3], "../src/bindings.js").unwrap();
//! }
//! ```
//!
//! ## Use on frontend
//!
//! ```ts
//! import * as commands from "./bindings"; // This should point to the file we export from Rust
//!
//! await commands.greet("Brendan");
//! ```
//!
#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::unwrap_used, clippy::panic, missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

/// The exporter for [Javascript](https://www.javascript.com).
#[cfg(feature = "javascript")]
#[cfg_attr(docsrs, doc(cfg(feature = "javascript")))]
pub mod js;

/// The exporter for [TypeScript](https://www.typescriptlang.org).
#[cfg(feature = "typescript")]
#[cfg_attr(docsrs, doc(cfg(feature = "typescript")))]
pub mod ts;

/// This remains for backwards compatibility. Please use [`specta::collect_types`] instead!
#[macro_export]
#[deprecated(
    note = "Please use `specta::collect_types` instead! This alias will be removed in a future release."
)]
macro_rules! collate_types {
    (type_map: $type_map:ident, $($command:path),*) => {{
        specta::functions::collect_types!(type_map: $type_map, $($command),*)
    }};
    ($($command:path),*) => {{
        let mut type_map = specta::TypeDefs::default();
        specta::functions::collect_types!(type_map: type_map, $($command),*)
    }};
}

// TODO
// #[cfg(doctest)]
// doc_comment::doctest!("../README.md");
