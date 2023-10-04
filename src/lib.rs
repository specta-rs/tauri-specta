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
#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::unwrap_used, clippy::panic
	// , missing_docs
)]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::{
    borrow::Cow,
    fs::{self, File},
    io::Write,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use specta::{
    functions::{CollectFunctionsResult, FunctionDataType},
    ts::ExportError,
    TypeMap,
};

use tauri::{Invoke, Manager, Runtime};
pub use tauri_specta_macros::Event;

/// The exporter for [Javascript](https://www.javascript.com).
#[cfg(feature = "javascript")]
#[cfg_attr(docsrs, doc(cfg(feature = "javascript")))]
pub mod js;

/// The exporter for [TypeScript](https://www.typescriptlang.org).
#[cfg(feature = "typescript")]
#[cfg_attr(docsrs, doc(cfg(feature = "typescript")))]
pub mod ts;

#[cfg(any(feature = "javascript", feature = "typescript"))]
mod js_ts;

mod event;

pub use event::*;

pub type CollectCommandsTuple<TInvokeHandler> =
    (specta::functions::CollectFunctionsResult, TInvokeHandler);

#[macro_export]
macro_rules! collect_commands {
 	(type_map: $type_map:ident, $($command:path),*) => {
        (
        	specta::collect_functions![$type_map; $($command),*],
       		::tauri::generate_handler![$($command),*],
        )
    };
    ($($command:path),*) => {{
        let mut type_map = specta::TypeMap::default();
        $crate::collect_commands![type_map: type_map, $($command),*]
    }};
}

// TODO
// #[cfg(doctest)]
// doc_comment::doctest!("../README.md");

/// A set of functions that produce language-specific code
pub trait ExportLanguage: 'static {
    type Config: Default + Clone;

    fn run_format(path: PathBuf, cfg: &ExportConfig<Self::Config>);

    fn render_events(
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig<Self::Config>,
    ) -> Result<String, ExportError>;

    /// Renders a collection of [`FunctionDataType`] into a string.
    fn render_commands(
        commands: &[FunctionDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig<Self::Config>,
    ) -> Result<String, ExportError>;

    /// Renders the output of [`globals`], [`render_functions`] and all dependant types into a TypeScript string.
    fn render(
        commands: &[FunctionDataType],
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig<Self::Config>,
    ) -> Result<String, ExportError>;
}

pub trait CommandsTypeState: 'static {
    type Runtime: tauri::Runtime;
    type InvokeHandler: Fn(tauri::Invoke<Self::Runtime>) + Send + Sync + 'static;

    fn split(self) -> CollectCommandsTuple<Self::InvokeHandler>;

    fn macro_data(&self) -> &CollectFunctionsResult;
}

fn dummy_invoke_handler(_: Invoke<impl Runtime>) {}

pub struct NoCommands<TRuntime>(CollectFunctionsResult, PhantomData<TRuntime>);

impl<TRuntime> CommandsTypeState for NoCommands<TRuntime>
where
    TRuntime: tauri::Runtime,
{
    type Runtime = TRuntime;
    type InvokeHandler = fn(Invoke<TRuntime>);

    fn split(self) -> CollectCommandsTuple<Self::InvokeHandler> {
        (Default::default(), dummy_invoke_handler)
    }

    fn macro_data(&self) -> &CollectFunctionsResult {
        &self.0
    }
}

pub struct Commands<TRuntime, TInvokeHandler>(
    CollectCommandsTuple<TInvokeHandler>,
    PhantomData<TRuntime>,
);

impl<TRuntime, TInvokeHandler> CommandsTypeState for Commands<TRuntime, TInvokeHandler>
where
    TRuntime: tauri::Runtime,
    TInvokeHandler: Fn(tauri::Invoke<TRuntime>) + Send + Sync + 'static,
{
    type Runtime = TRuntime;
    type InvokeHandler = TInvokeHandler;

    fn split(self) -> CollectCommandsTuple<TInvokeHandler> {
        self.0
    }

    fn macro_data(&self) -> &CollectFunctionsResult {
        &self.0 .0
    }
}

pub trait EventsTypeState: 'static {
    fn get(self) -> CollectEventsTuple;
}

pub struct NoEvents;

impl EventsTypeState for NoEvents {
    fn get(self) -> CollectEventsTuple {
        Default::default()
    }
}

pub struct Events(CollectEventsTuple);

impl EventsTypeState for Events {
    fn get(self) -> CollectEventsTuple {
        self.0
    }
}

/// General exporter, takes a generic for the specific language that is being exported to.
pub struct PluginBuilder<TLang: ExportLanguage, TCommands, TEvents> {
    lang: PhantomData<TLang>,
    commands: TCommands,
    events: TEvents,
    config: ExportConfig<TLang::Config>,
}

impl<TLang, TRuntime> Default for PluginBuilder<TLang, NoCommands<TRuntime>, NoEvents>
where
    TLang: ExportLanguage,
{
    fn default() -> Self {
        Self {
            lang: PhantomData,
            commands: NoCommands(Default::default(), Default::default()),
            events: NoEvents,
            config: Default::default(),
        }
    }
}

impl<TLang, TEvents, TRuntime> PluginBuilder<TLang, NoCommands<TRuntime>, TEvents>
where
    TLang: ExportLanguage,
    TRuntime: tauri::Runtime,
{
    pub fn commands<TInvokeHandler: Fn(tauri::Invoke<TRuntime>) + Send + Sync + 'static>(
        self,
        commands: CollectCommandsTuple<TInvokeHandler>,
    ) -> PluginBuilder<TLang, Commands<TRuntime, TInvokeHandler>, TEvents> {
        PluginBuilder {
            lang: self.lang,
            commands: Commands(commands, Default::default()),
            events: self.events,
            config: self.config,
        }
    }
}

impl<TLang, TCommands> PluginBuilder<TLang, TCommands, NoEvents>
where
    TLang: ExportLanguage,
{
    pub fn events(self, events: CollectEventsTuple) -> PluginBuilder<TLang, TCommands, Events> {
        PluginBuilder {
            lang: self.lang,
            events: Events(events),
            commands: self.commands,
            config: self.config,
        }
    }
}

impl<TLang, TCommands, TEvents> PluginBuilder<TLang, TCommands, TEvents>
where
    TLang: ExportLanguage,
{
    /// Allows for specifying a custom [`ExportConfiguration`](specta::ts::ExportConfiguration).
    pub fn config(mut self, config: TLang::Config) -> Self {
        self.config.inner = config;
        self
    }

    /// Allows for specifying a custom header to
    pub fn header(mut self, header: &'static str) -> Self {
        self.config.header = header.into();
        self
    }

    pub fn path(mut self, path: impl AsRef<Path>) -> Self {
        self.config.path = Some(path.as_ref().to_path_buf());
        self
    }
}

pub struct PluginUtils<TCommands, TManager, TSetup>
where
    TCommands: CommandsTypeState,
    TManager: Manager<TCommands::Runtime>,
    TSetup: FnOnce(&TManager),
{
    pub invoke_handler: TCommands::InvokeHandler,
    pub setup: TSetup,
    phantom: PhantomData<TManager>,
}

const PLUGIN_NAME: &str = "tauri-specta";

impl<TLang, TCommands, TEvents> PluginBuilder<TLang, TCommands, TEvents>
where
    TLang: ExportLanguage,
    TCommands: CommandsTypeState,
    TEvents: EventsTypeState,
{
    #[must_use]
    pub fn into_plugin(self) -> tauri::plugin::TauriPlugin<TCommands::Runtime> {
        let builder = tauri::plugin::Builder::new(PLUGIN_NAME);

        let plugin_utils = self.into_plugin_utils(PLUGIN_NAME);

        builder
            .invoke_handler(plugin_utils.invoke_handler)
            .setup(move |app| {
                (plugin_utils.setup)(app);

                Ok(())
            })
            .build()
    }

    #[must_use]
    pub fn into_plugin_utils<TManager>(
        mut self,
        plugin_name: &'static str,
    ) -> PluginUtils<TCommands, TManager, impl FnOnce(&TManager)>
    where
        TManager: Manager<TCommands::Runtime>,
    {
        let plugin_name = PluginName::new(plugin_name);

        self.config.plugin_name = plugin_name;

        let (invoke_handler, event_collection) = self.export_inner().unwrap();

        PluginUtils {
            invoke_handler,
            setup: move |app| {
                let registry = EventRegistry::get_or_manage(app);
                registry.register_collection(event_collection, plugin_name);
            },
            phantom: PhantomData,
        }
    }

    fn export_inner(self) -> Result<(TCommands::InvokeHandler, EventCollection), ExportError> {
        let cfg = self.config.clone();

        let (rendered, ret) = self.render()?;

        if let Some(path) = cfg.path.clone() {
            if let Some(export_dir) = path.parent() {
                fs::create_dir_all(export_dir)?;
            }

            let mut file = File::create(&path)?;

            write!(file, "{}", rendered)?;

            TLang::run_format(path, &cfg);
        }

        Ok(ret)
    }

    fn render(self) -> Result<(String, (TCommands::InvokeHandler, EventCollection)), ExportError> {
        let Self {
            commands,
            config,
            events,
            ..
        } = self;

        let ((commands, commands_type_map), invoke_handler) = commands.split();

        let (events_registry, events, events_type_map) = events.get();

        let rendered = TLang::render(
            &commands,
            &events,
            &commands_type_map
                .into_iter()
                .chain(events_type_map)
                .collect(),
            &config,
        )?;

        Ok((
            format!("{}{rendered}", &config.header),
            (invoke_handler, events_registry),
        ))
    }
}

type HardcodedRuntime = tauri::Wry;

impl<TLang, TCommands, TEvents> PluginBuilder<TLang, TCommands, TEvents>
where
    TLang: ExportLanguage,
    TCommands: CommandsTypeState<Runtime = HardcodedRuntime>,
    TEvents: EventsTypeState,
{
    /// Exports the output of [`internal::render`] for a collection of [`FunctionDataType`] into a TypeScript file.
    pub fn export(self) -> Result<(), specta::ts::ExportError> {
        self.export_for_plugin(PLUGIN_NAME)
    }

    pub fn export_for_plugin(
        mut self,
        plugin_name: &'static str,
    ) -> Result<(), specta::ts::ExportError> {
        self.config.plugin_name = PluginName::new(plugin_name);

        self.export_inner().map(|_| ())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PluginName(&'static str);

pub(crate) enum ItemType {
    Event,
    Command,
}

impl Default for PluginName {
    fn default() -> Self {
        PluginName(PLUGIN_NAME)
    }
}

impl PluginName {
    pub fn new(plugin_name: &'static str) -> Self {
        Self(plugin_name)
    }

    pub fn apply_as_prefix(&self, s: &str, item_type: ItemType) -> String {
        format!(
            "plugin:{}{}{}",
            self.0,
            match item_type {
                ItemType::Event => ":",
                ItemType::Command => "|",
            },
            s,
        )
    }
}

/// The configuration for the generator
#[derive(Default, Clone)]
pub struct ExportConfig<TConfig> {
    /// The name of the plugin to invoke.
    ///
    /// If there is no plugin name (i.e. this is an app), this should be `None`.
    pub(crate) plugin_name: PluginName,
    /// The specta export configuration
    pub(crate) inner: TConfig,
    pub(crate) path: Option<PathBuf>,
    pub(crate) header: Cow<'static, str>,
}

impl<TConfig: Default> ExportConfig<TConfig> {
    /// Creates a new [`ExportConfiguration`] from a [`specta::ts::ExportConfiguration`]
    pub fn new(config: TConfig) -> Self {
        Self {
            inner: config,
            ..Default::default()
        }
    }
}
