//! TODO
//!
//! Known Issues:
//!  - You can assign commands, events, types, constants after `From` into builder which will override. Fine for now.

use std::{borrow::Cow, collections::BTreeMap};

use specta::Types;
use tauri::Runtime;
use tauri_specta::{Commands, Events};

pub struct CommandSet<R: Runtime> {
    queries: Commands<R>,
    mutations: Commands<R>,
    events: Events,
    types: Types,
    constants: BTreeMap<Cow<'static, str>, serde_json::Value>,
}

impl<R: Runtime> CommandSet<R> {
    pub fn new(queries: Commands<R>, mutations: Commands<R>) -> Self {
        Self {
            queries,
            mutations,
            events: Default::default(),
            types: Default::default(),
            constants: Default::default(),
        }
    }

    pub fn events(self, events: Events) -> Self {
        Self { events, ..self }
    }

    pub fn types(self, types: Types) -> Self {
        Self { types, ..self }
    }

    pub fn constants(self, constants: BTreeMap<Cow<'static, str>, serde_json::Value>) -> Self {
        Self { constants, ..self }
    }

    pub fn merge(&self, other: &Self) -> Self {
        todo!();
        // Self {
        //     queries: self.queries.merge(&other.queries),
        //     mutations: self.mutations.merge(&other.mutations),
        //     events: self.events.merge(&other.events),
        //     types: self.types.merge(&other.types),
        //     constants: self.constants.merge(&other.constants),
        // }
    }
}

impl<R: Runtime> From<CommandSet<R>> for tauri_specta::Builder<R> {
    fn from(value: CommandSet<R>) -> Self {
        tauri_specta::Builder::<R>::new()
            // TODO: This will cause problems cause these don't merge
            // .commands(value.queries)
            .commands(value.mutations)
            .events(value.events)
            .constant("testing", "hello world")

        // TODO
        // .types(value.types)
        // .constants(value.constants)
    }
}
