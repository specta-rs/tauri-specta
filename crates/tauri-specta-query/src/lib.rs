//! Generate [TanStack Query](https://tanstack.com/query/latest) helpers for
//! commands exported by [`tauri-specta`](https://docs.rs/tauri-specta).
//!
//! This crate lets a Tauri application classify its commands as either
//! **queries** (operations that read data) or **mutations** (operations that
//! change data). [`CommandSet`] combines both groups into one Tauri invoke
//! handler and generates framework-specific query options, mutation options,
//! and cache keys alongside the normal `tauri-specta` bindings.
//!
//! # Backend setup
//!
//! Annotate commands with both `#[tauri::command]` and `#[specta::specta]`, then
//! pass separate command collections to [`CommandSet::new`]. The string returned
//! by [`CommandSet::build`] must be supplied to the TypeScript exporter with
//! `Typescript::with_raw`.
//!
//! ```rust,no_run
//! use specta_typescript::Typescript;
//! use tauri_specta::collect_commands;
//! use tauri_specta_query::{CommandSet, TanstackQueryFramework};
//!
//! #[tauri::command]
//! #[specta::specta]
//! fn get_user(id: u32) -> String {
//!     format!("user-{id}")
//! }
//!
//! #[tauri::command]
//! #[specta::specta]
//! fn rename_user(id: u32, name: String) {
//!     // Update the user in your application state.
//! }
//!
//! let commands = CommandSet::<tauri::Wry>::new(
//!     collect_commands![get_user],
//!     collect_commands![rename_user],
//! );
//! let (query_bindings, builder) = commands.build(TanstackQueryFramework::React);
//!
//! #[cfg(debug_assertions)]
//! builder
//!     .export(
//!         Typescript::default().with_raw(query_bindings),
//!         "../src/bindings.ts",
//!     )
//!     .expect("failed to export bindings");
//!
//! tauri::Builder::default()
//!     .invoke_handler(builder.invoke_handler())
//!     .setup(move |app| {
//!         builder.mount_events(app);
//!         Ok(())
//!     });
//! ```
//!
//! In a complete application, finish the Tauri builder with `run` as usual.
//!
//! # Frontend usage
//!
//! The generated module exports four objects:
//!
//! - `queries`, whose methods accept the same arguments as the corresponding
//!   command and return query options;
//! - `mutations`, whose zero-argument methods return mutation options;
//! - `queryKeys`, for matching or invalidating query caches (its arguments are
//!   partial, so prefixes can be constructed); and
//! - `mutationKeys`, for matching mutation caches.
//!
//! For React Query, the commands above can be consumed like this:
//!
//! ```tsx
//! import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
//! import { mutations, queries, queryKeys } from "./bindings";
//!
//! const user = useQuery(queries.getUser(1));
//! const queryClient = useQueryClient();
//! const renameUser = useMutation({
//!   ...mutations.renameUser(),
//!   onSuccess: () => queryClient.invalidateQueries({ queryKey: queryKeys.getUser(1) }),
//! });
//!
//! renameUser.mutate({ id: 1, name: "Ada" });
//! ```
//!
//! Command names and argument names are converted to `lowerCamelCase` in the
//! generated TypeScript. Mutation arguments are passed to `mutate` as one object;
//! a mutation command with no arguments instead receives no input.
//!
//! # Frameworks
//!
//! Choose the matching [`TanstackQueryFramework`] variant for React, Solid, Vue,
//! Angular, Svelte, or Preact. The generated helpers use that framework's
//! TanStack Query package. Svelte helpers return option-producing functions and
//! do not generate an import.
//!
//! # Additional bindings
//!
//! Register events, standalone types, and constants on [`CommandSet`] before
//! calling [`CommandSet::build`]. The returned `tauri_specta::Builder` can then
//! be configured further—for example with semantic types—before it is exported
//! and mounted.

use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use heck::ToLowerCamelCase;
use serde::Serialize;
use specta::{Type, Types, datatype};
use tauri::{Runtime, ipc::Invoke};
use tauri_specta::{Commands, Events};

/// A collection of query commands, mutation commands, and their shared bindings.
///
/// Build a command set with [`CommandSet::new`], optionally register additional
/// bindings, and finish it with [`CommandSet::build`].
pub struct CommandSet<R: Runtime> {
    handler: Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    queries: Vec<datatype::Function>,
    mutations: Vec<datatype::Function>,
    types: Types,
    events: Events,
    constants: BTreeMap<Cow<'static, str>, serde_json::Value>,
}

impl<R: Runtime> CommandSet<R> {
    /// Creates a command set from query commands and mutation commands.
    ///
    /// Both arguments are normally created with
    /// [`tauri_specta::collect_commands!`]. A command should appear in only one
    /// collection so it receives one unambiguous set of generated helpers.
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

    /// Registers the events to include in the generated bindings.
    ///
    /// This replaces events previously assigned to the command set.
    #[must_use]
    pub fn events(self, events: Events) -> Self {
        Self { events, ..self }
    }

    /// Replaces the type registry used by this command set.
    ///
    /// Prefer [`CommandSet::typ`] when adding individual standalone types. This
    /// method replaces the registry populated from the query and mutation
    /// commands, so the supplied registry must contain every required type.
    #[must_use]
    pub fn types(self, types: Types) -> Self {
        Self { types, ..self }
    }

    /// Registers a standalone Specta type in the generated bindings.
    ///
    /// Use this for types that are not already reachable from a command or
    /// event. It can be chained multiple times.
    #[must_use]
    pub fn typ<T: Type>(mut self) -> Self {
        self.types.register_mut::<T>();
        self
    }

    /// Adds a serializable constant to the generated bindings.
    ///
    /// Assigning the same key more than once retains the last value.
    ///
    /// # Panics
    ///
    /// Panics if `value` cannot be represented as a [`serde_json::Value`].
    #[track_caller]
    #[must_use]
    pub fn constant<T: Serialize>(mut self, k: impl Into<Cow<'static, str>>, v: T) -> Self {
        self.constants.insert(
            k.into(),
            serde_json::to_value(v).expect("Tauri Specta failed to serialize constant"),
        );
        self
    }

    /// Combines the commands and types from two command sets.
    ///
    /// # Panics
    ///
    /// Event and constant merging is not implemented yet, so this method
    /// currently panics. It should not be used until that support is added.
    #[must_use]
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

    /// Generates TanStack Query helpers and constructs the Tauri Specta builder.
    ///
    /// The returned string is a raw TypeScript fragment. Pass it to
    /// `specta_typescript::Typescript::with_raw` (or `JSDoc::with_raw`) when
    /// exporting the returned builder. The builder owns the combined invoke
    /// handler and must also be supplied to Tauri with
    /// [`tauri_specta::Builder::invoke_handler`].
    #[must_use]
    pub fn build(self, framework: TanstackQueryFramework) -> (String, tauri_specta::Builder<R>) {
        let output = {
            let mut output = format!("/** Tanstack Query */\n{}\n", framework.import());

            if !self.queries.is_empty() {
                output.push_str("\nexport const queries = {");
                for function in &self.queries {
                    let name = function.name.to_lower_camel_case();
                    let name_json =
                        serde_json::to_string(&name).expect("failed to serialize query name");

                    let options = format!(
                        "{{ queryKey: [{name_json}, ...args], queryFn: () => commands.{name}(...args) }}"
                    );

                    output.push_str(&format!(
                        "\n\t{name}: (...args: Parameters<typeof commands.{name}>) => {},",
                        framework.query_options(options)
                    ));
                }
                if !self.queries.is_empty() {
                    output.push('\n');
                }
                output.push_str("};");

                output.push_str("\nexport const queryKeys = {");
                for function in &self.queries {
                    let name = function.name.to_lower_camel_case();
                    let name_json =
                        serde_json::to_string(&name).expect("failed to serialize query name");

                    output.push_str(&format!(
                        "\n\t{name}: (...args: Partial<Parameters<typeof commands.{name}>>) => [{name_json}, ...args],"
                    ));
                }
                output.push_str("\n};");
            }

            if !self.mutations.is_empty() {
                output.push_str("\nexport const mutations = {");
                for function in &self.mutations {
                    let name = function.name.to_lower_camel_case();
                    let name_json =
                        serde_json::to_string(&name).expect("failed to serialize mutation name");
                    let args = function
                        .args
                        .iter()
                        .enumerate()
                        .map(|(idx, (arg, _))| {
                            let arg = arg.to_lower_camel_case();
                            format!("{arg}: Parameters<typeof commands.{name}>[{idx}]")
                        })
                        .collect::<Vec<_>>();
                    let arg_names = function
                        .args
                        .iter()
                        .map(|(arg, _)| arg.to_lower_camel_case())
                        .collect::<Vec<_>>();

                    let mutation_fn = if args.is_empty() {
                        format!("() => commands.{name}()")
                    } else {
                        format!(
                            "(input: {{ {} }}) => commands.{name}({})",
                            args.join("; "),
                            arg_names
                                .iter()
                                .map(|arg| format!("input.{arg}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };

                    let options =
                        format!("{{ mutationKey: [{name_json}], mutationFn: {mutation_fn} }}");

                    output.push_str(&format!(
                        "\n\t{name}: () => {},",
                        framework.mutation_options(options)
                    ));
                }
                if !self.mutations.is_empty() {
                    output.push('\n');
                }
                output.push_str("};");

                output.push_str("\nexport const mutationKeys = {");
                for function in &self.mutations {
                    let name = function.name.to_lower_camel_case();
                    let name_json =
                        serde_json::to_string(&name).expect("failed to serialize mutation name");

                    output.push_str(&format!("\n\t{name}: () => [{name_json}],"));
                }
                output.push_str("\n};");
            }

            if !self.queries.is_empty() && !self.mutations.is_empty() {
                output.push('\n');
            }

            output
        };

        let mut commands = self.queries;
        commands.extend(self.mutations);
        let types = self.types;

        let mut builder = tauri_specta::Builder::<R>::new()
            .commands(Commands(
                self.handler,
                Arc::new(move |tys| {
                    tys.extend(&types);
                    commands.clone()
                }),
            ))
            .events(self.events);

        for (k, v) in self.constants {
            builder = builder.constant(k, v);
        }

        (output, builder)
    }
}

/// The frontend framework targeted by the generated TanStack Query helpers.
///
/// [`React`](Self::React) is the default.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TanstackQueryFramework {
    /// React, using `@tanstack/react-query`.
    #[default]
    React,
    /// Solid, using `@tanstack/solid-query`.
    Solid,
    /// Vue, using `@tanstack/vue-query`.
    Vue,
    /// Angular, using `@tanstack/angular-query-experimental`.
    Angular,
    /// Svelte, emitting option-producing functions without an import.
    Svelte,
    /// Preact, using `@tanstack/preact-query`.
    Preact,
}

impl TanstackQueryFramework {
    fn import(self) -> &'static str {
        match self {
            Self::React => "import { mutationOptions, queryOptions } from '@tanstack/react-query';",
            Self::Solid => "import { mutationOptions, queryOptions } from '@tanstack/solid-query';",
            Self::Vue => "import { mutationOptions, queryOptions } from '@tanstack/vue-query';",
            Self::Angular => {
                "import { mutationOptions, queryOptions } from '@tanstack/angular-query-experimental';"
            }
            Self::Svelte => "",
            Self::Preact => {
                "import { mutationOptions, queryOptions } from '@tanstack/preact-query';"
            }
        }
    }

    fn query_options(self, options: String) -> String {
        match self {
            Self::Svelte => format!("() => ({options})"),
            _ => format!("queryOptions({options})"),
        }
    }

    fn mutation_options(self, options: String) -> String {
        match self {
            Self::Svelte => format!("() => ({options})"),
            _ => format!("mutationOptions({options})"),
        }
    }
}
