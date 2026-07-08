//! TODO
//!
//! Known Issues:
//!  - You can assign commands, events, types, constants after `From` into builder which will override. Fine for now.

use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use heck::ToLowerCamelCase;
use serde::Serialize;
use specta::{Type, Types, datatype};
use tauri::{Runtime, ipc::Invoke};
use tauri_specta::{Commands, Events};

pub struct CommandSet<R: Runtime> {
    handler: Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    queries: Vec<datatype::Function>,
    mutations: Vec<datatype::Function>,
    types: Types,
    events: Events,
    constants: BTreeMap<Cow<'static, str>, serde_json::Value>,
}

impl<R: Runtime> CommandSet<R> {
    pub fn new(queries: Commands<R>, mutations: Commands<R>) -> Self {
        let Commands(query_invoke, query_types) = queries;
        let Commands(mutation_invoke, mutation_types) = mutations;

        let mut types = Types::default();
        let queries = query_types(&mut types);
        let mutations = mutation_types(&mut types);

        Self {
            handler: Arc::new(move |i| {
                (query_invoke)(Invoke {
                    message: i.message.clone(),
                    resolver: i.resolver.clone(),
                    acl: i.acl.clone(),
                }) || (mutation_invoke)(i)
            }),
            queries,
            mutations,
            types,
            events: Default::default(),
            constants: Default::default(),
        }
    }

    pub fn events(self, events: Events) -> Self {
        Self { events, ..self }
    }

    pub fn types(self, types: Types) -> Self {
        Self { types, ..self }
    }

    pub fn typ<T: Type>(mut self) -> Self {
        self.types.register_mut::<T>();
        self
    }

    #[track_caller]
    pub fn constant<T: Serialize>(mut self, k: impl Into<Cow<'static, str>>, v: T) -> Self {
        self.constants.insert(
            k.into(),
            serde_json::to_value(v).expect("Tauri Specta failed to serialize constant"),
        );
        self
    }

    pub fn merge(&self, other: &Self) -> Self {
        let mut types = self.types.clone();
        types.extend(&other.types);

        Self {
            handler: Arc::new(move |i| {
                (self.handler)(Invoke {
                    message: i.message.clone(),
                    resolver: i.resolver.clone(),
                    acl: i.acl.clone(),
                }) || (other.handler)(i)
            }),
            queries: self
                .queries
                .iter()
                .chain(other.queries.iter())
                .cloned()
                .collect(),
            mutations: self
                .mutations
                .iter()
                .chain(other.mutations.iter())
                .cloned()
                .collect(),
            types,
            events: todo!(),    // self.events.merge(&other.events),
            constants: todo!(), // self.constants.merge(&other.constants),
        }
    }

    pub fn build(self, framework: TanstackQueryFramework) -> (String, tauri_specta::Builder<R>) {
        let output = {
            let mut output = format!("/** Tanstack Query */\n{}\n", framework.import());

            if !self.queries.is_empty() {
                output.push_str("\nexport const queries = {");
                for function in &self.queries {
                    let name = function.name.to_lower_camel_case();
                    let name_json =
                        serde_json::to_string(&name).expect("failed to serialize query name");

                    output.push_str(&format!(
                        "\n\t{name}: (...args: Parameters<typeof commands.{name}>) => queryOptions({{ queryKey: [{name_json}, ...args], queryFn: () => commands.{name}(...args) }}),"
                    ));
                }
                if !self.queries.is_empty() {
                    output.push('\n');
                }
                output.push_str("};");
            }

            if !self.mutations.is_empty() {
                output.push_str("\nexport const mutations = {");
                for function in &self.mutations {
                    let name = function.name.to_lower_camel_case();
                    let name_json =
                        serde_json::to_string(&name).expect("failed to serialize mutation name");

                    output.push_str(&format!(
                        "\n\t{name}: mutationOptions({{ mutationKey: [{name_json}], mutationFn: (input: Parameters<typeof commands.{name}>) => commands.{name}(...input) }}),"
                    ));
                }
                if !self.mutations.is_empty() {
                    output.push('\n');
                }
                output.push_str("};");
            }

            if !self.queries.is_empty() && !self.mutations.is_empty() {
                output.push('\n');
            }

            output
        };

        let mut commands = self.queries;
        commands.extend(self.mutations);

        let mut builder = tauri_specta::Builder::<R>::new()
            // TODO: Removing `internal_commands` in favor of `commands`
            .internal_commands(Commands(self.handler, |_| Default::default()), commands)
            .events(self.events)
            .types(&self.types);

        for (k, v) in self.constants {
            builder = builder.constant(k, v);
        }

        (output, builder)
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TanstackQueryFramework {
    #[default]
    React,
    // TODO: Rest of them
}

impl TanstackQueryFramework {
    fn import(self) -> &'static str {
        match self {
            Self::React => "import { mutationOptions, queryOptions } from '@tanstack/react-query';",
        }
    }
}
