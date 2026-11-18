//! Pins [Tauri Specta](https://docs.rs/tauri-specta)'s Tauri dependency to v2.
//!
//! This crate has no API. It exists only so `tauri-specta`'s default `v2` feature
//! can put a real `tauri = "2"` edge into the dependency graph.
//!
//! `tauri-specta` accepts Tauri v2 and tested v3 prereleases through one
//! implementation. Cargo picks the highest version a requirement allows, so
//! without a constraint an application that does not depend on Tauri directly
//! would resolve a v3 alpha. Enabling this crate constrains that shared `tauri`
//! package back to stable v2.
//!
//! Applications that opt into Tauri v3 disable `tauri-specta`'s default features,
//! which removes this constraint, and add the matching Tauri v3 dependency
//! themselves.
//!
//! ```toml
//! # Default: stable Tauri v2, even with no other Tauri dependency.
//! tauri-specta = { version = "2", features = ["derive", "typescript"] }
//!
//! # Tauri v3 prerelease: drop the pin and select the major yourself.
//! tauri-specta = { version = "2", default-features = false, features = ["derive", "typescript"] }
//! tauri = "=3.0.0-alpha.6"
//! ```

// The dependency is the entire purpose of this crate.
use tauri as _;
