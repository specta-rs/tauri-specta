// TODO: Restrict this to only non-JS/TS languages.

use std::{
    borrow::{Borrow, Cow},
    collections::HashMap,
};

use serde::Serialize;

/// Define a set of statics which can be included in the exporter
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StaticCollection {
    pub(crate) statics: HashMap<Cow<'static, str>, serde_json::Value>,
}

impl StaticCollection {
    /// Join another type collection into this one.
    pub fn extend(&mut self, collection: impl Borrow<StaticCollection>) -> &mut Self {
        self.statics.extend(
            collection
                .borrow()
                .statics
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        self
    }

    /// Register a static with the collection.
    pub fn register<T: Serialize>(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        value: T,
    ) -> &mut Self {
        self.statics
            .insert(name.into(), serde_json::to_value(&value).unwrap());
        self
    }
}
