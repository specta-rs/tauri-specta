//! An application-shaped crate that never mentions `tauri` itself.
//!
//! This is the case the default `v2` feature exists for: a code-generation or
//! helper crate that depends on Tauri Specta but has no Tauri dependency of its
//! own. Its committed lockfile must contain stable Tauri v2, not a v3 alpha.
//! `tests/assert_tauri_version.py` enforces that in CI.
//!
//! Naming a `tauri` type here would defeat the purpose, so the crate only proves
//! the graph resolves and the library compiles against the version the default
//! feature selected.

/// Marker so the crate has a documented public item.
pub const DEFAULT_FEATURES_SELECT_TAURI_V2: &str = "see tests/assert_tauri_version.py";

/// Re-exported so the fixture also covers `tauri-specta-query`'s default `v2`.
pub use tauri_specta_query as query;
