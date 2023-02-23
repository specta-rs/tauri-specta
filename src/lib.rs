pub mod js;
pub mod ts;

#[deprecated = "Please use `specta::collect_types` instead! This alias will be removed in a future release."]
pub use specta::collect_types as collate_types;
