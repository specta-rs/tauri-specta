//! Export the shared compatibility fixture against the main workspace's Tauri v2.

#![allow(missing_docs, dead_code, clippy::unwrap_used)]

#[path = "../../../tests/support/semantic_channels.rs"]
mod semantic_channels;
#[path = "../../../tests/support/version_parity.rs"]
mod version_parity;

#[test]
fn exports_v2_bindings_for_version_parity() {
    // Stable location for CI artifact upload, independent of CARGO_TARGET_DIR.
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/version-parity/v2");
    std::fs::create_dir_all(&dir).unwrap();
    let stale = dir.join("stale-from-cache.txt");
    std::fs::write(&stale, "not a current export").unwrap();
    version_parity::export(&dir);
    assert!(
        !stale.exists(),
        "cached files must not survive regeneration"
    );
}
