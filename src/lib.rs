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
    borrow::{Borrow, Cow},
    error, fmt,
    fs::{self, File},
    io::Write,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use specta::{
    function::{CollectFunctionsResult, FunctionDataType},
    NamedDataType, SpectaID, TypeCollection, TypeMap,
};

use tauri::{ipc::Invoke, Manager, Runtime};
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
mod manager_ext;
mod statics;

pub use event::*;
pub use manager_ext::*;
pub use statics::StaticCollection;

pub type CollectCommandsTuple<TInvokeHandler> =
    (specta::function::CollectFunctionsResult, TInvokeHandler);

pub use tauri_specta_macros::collect_commands;

/// A set of functions that produce language-specific code
pub trait ExportLanguage: 'static {
    type Config: Default + Clone;
    type Error: fmt::Debug + error::Error + From<std::io::Error>; // TODO: Review if this `From<std::io::Error>` should be removed before stabilisation

    fn run_format(path: PathBuf, cfg: &ExportConfig<Self::Config>);

    fn render_events(
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig<Self::Config>,
    ) -> Result<String, Self::Error>;

    /// Renders a collection of [`FunctionDataType`] into a string.
    fn render_commands(
        commands: &[FunctionDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig<Self::Config>,
    ) -> Result<String, Self::Error>;

    /// Renders the output of [`globals`], [`render_functions`] and all dependant types into a TypeScript string.
    fn render(
        commands: &[FunctionDataType],
        events: &[EventDataType],
        type_map: &TypeMap,
        statics: &StaticCollection,
        cfg: &ExportConfig<Self::Config>,
    ) -> Result<String, Self::Error>;
}

pub trait CommandsTypeState: 'static {
    type Runtime: tauri::Runtime;
    type InvokeHandler: Fn(Invoke<Self::Runtime>) -> bool + Send + Sync + 'static;

    fn split(self) -> CollectCommandsTuple<Self::InvokeHandler>;

    fn macro_data(&self) -> &CollectFunctionsResult;
}

fn dummy_invoke_handler(_: Invoke<impl Runtime>) -> bool {
    false
}

pub struct NoCommands<TRuntime>(CollectFunctionsResult, PhantomData<TRuntime>);

impl<TRuntime> CommandsTypeState for NoCommands<TRuntime>
where
    TRuntime: tauri::Runtime,
{
    type Runtime = TRuntime;
    type InvokeHandler = fn(Invoke<TRuntime>) -> bool;

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
    TInvokeHandler: Fn(Invoke<TRuntime>) -> bool + Send + Sync + 'static,
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
pub struct Builder<TLang: ExportLanguage, TCommands, TEvents> {
    lang: PhantomData<TLang>,
    commands: TCommands,
    events: TEvents,
    config: ExportConfig<TLang::Config>,
    types: TypeCollection,
    statics: StaticCollection,
}

impl<TLang, TRuntime> Default for Builder<TLang, NoCommands<TRuntime>, NoEvents>
where
    TLang: ExportLanguage,
{
    fn default() -> Self {
        Self {
            lang: PhantomData,
            commands: NoCommands(Default::default(), Default::default()),
            events: NoEvents,
            config: Default::default(),
            types: TypeCollection::default(),
            statics: StaticCollection::default(),
        }
    }
}

impl<TLang, TEvents, TRuntime> Builder<TLang, NoCommands<TRuntime>, TEvents>
where
    TLang: ExportLanguage,
    TRuntime: tauri::Runtime,
{
    pub fn commands<TInvokeHandler: Fn(Invoke<TRuntime>) -> bool + Send + Sync + 'static>(
        self,
        commands: CollectCommandsTuple<TInvokeHandler>,
    ) -> Builder<TLang, Commands<TRuntime, TInvokeHandler>, TEvents> {
        Builder {
            lang: self.lang,
            commands: Commands(commands, Default::default()),
            events: self.events,
            config: self.config,
            types: self.types,
            statics: self.statics,
        }
    }
}

impl<TLang, TCommands> Builder<TLang, TCommands, NoEvents>
where
    TLang: ExportLanguage,
{
    pub fn events(self, events: CollectEventsTuple) -> Builder<TLang, TCommands, Events> {
        Builder {
            lang: self.lang,
            events: Events(events),
            commands: self.commands,
            config: self.config,
            types: self.types,
            statics: self.statics,
        }
    }
}

impl<TLang, TCommands, TEvents> Builder<TLang, TCommands, TEvents>
where
    TLang: ExportLanguage,
{
    /// Allows for exporting types that are not part of any of the commands or events.
    ///
    /// ```rs
    /// use tauri_specta::ts;
    /// use specta::{Type, TypeCollection};
    ///
    /// #[derive(Type)]
    /// pub struct Custom(String);
    ///
    /// ts::build()
    ///    .types({
    ///         let mut collection = TypeCollection::default();
    ///         collection.register::<Custom>();
    ///         collection
    ///    });
    /// ```
    pub fn types(mut self, types: impl Borrow<TypeCollection>) -> Self {
        self.types.extend(types);
        self
    }

    /// Allows for exporting static along with your commands and events.
    ///
    /// ```rs
    /// use tauri_specta::{ts, StaticCollection};
    ///
    /// ts::build()
    ///    .statics(StaticCollection::default().register("universalConstant", 42));
    /// ```
    pub fn statics(mut self, statics: impl Borrow<StaticCollection>) -> Self {
        self.statics.extend(statics);
        self
    }

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

impl<TLang, TCommands, TEvents> Builder<TLang, TCommands, TEvents>
where
    TLang: ExportLanguage,
    TCommands: CommandsTypeState,
    TEvents: EventsTypeState,
{
    fn build_inner(self) -> Result<(TCommands::InvokeHandler, EventCollection), TLang::Error> {
        let cfg = self.config.clone();

        let (rendered, (invoke_handler, events)) = self.render()?;

        if let Some(path) = cfg.path.clone() {
            if let Some(export_dir) = path.parent() {
                fs::create_dir_all(export_dir)?;
            }

            let mut file = File::create(&path)?;

            write!(file, "{}", rendered)?;

            TLang::run_format(path, &cfg);
        }

        Ok((invoke_handler, events))
    }

    fn render(self) -> Result<(String, (TCommands::InvokeHandler, EventCollection)), TLang::Error> {
        let Self {
            commands,
            config,
            events,
            types,
            statics,
            ..
        } = self;

        let ((commands, commands_type_map), invoke_handler) = commands.split();

        let (events_registry, events, events_type_map) = events.get();

        let mut type_map = collect_typemap(commands_type_map.iter().chain(events_type_map.iter()));
        types.export(&mut type_map);

        let rendered = TLang::render(&commands, &events, &type_map, &statics, &config)?;

        Ok((
            format!("{}\n{rendered}", &config.header),
            (invoke_handler, events_registry),
        ))
    }
}

impl<TLang, TCommands> Builder<TLang, TCommands, NoEvents>
where
    TLang: ExportLanguage,
    TCommands: CommandsTypeState,
{
    #[must_use]
    pub fn build_plugin_utils(
        mut self,
        plugin_name: &'static str,
    ) -> Result<TCommands::InvokeHandler, TLang::Error> {
        let plugin_name = PluginName::new(plugin_name);

        self.config.plugin_name = Some(plugin_name);

        Ok(self.build_inner()?.0)
    }

    #[must_use]
    pub fn build(self) -> Result<TCommands::InvokeHandler, TLang::Error> {
        Ok(self.build_inner()?.0)
    }
}

impl<TLang, TCommands> Builder<TLang, TCommands, Events>
where
    TLang: ExportLanguage,
    TCommands: CommandsTypeState,
{
    #[must_use]
    pub fn build_plugin_utils<TManager: Manager<TCommands::Runtime>>(
        mut self,
        plugin_name: &'static str,
    ) -> Result<(TCommands::InvokeHandler, impl FnOnce(&TManager)), TLang::Error> {
        let plugin_name = PluginName::new(plugin_name);

        self.config.plugin_name = Some(plugin_name);

        let (invoke_handler, event_collection) = self.build_inner()?;

        Ok((invoke_handler, move |app: &_| {
            let registry = EventRegistry::get_or_manage(app);
            registry.register_collection(event_collection, Some(plugin_name));
        }))
    }

    #[must_use]
    pub fn build<TManager: Manager<TCommands::Runtime>>(
        self,
    ) -> Result<(TCommands::InvokeHandler, impl FnOnce(&TManager)), TLang::Error> {
        let (invoke_handler, event_collection) = self.build_inner()?;

        Ok((invoke_handler, move |app: &_| {
            let registry = EventRegistry::get_or_manage(app);
            registry.register_collection(event_collection, None);
        }))
    }
}

// TODO: Add a proper solution to this into Specta
fn collect_typemap<'a>(iter: impl Iterator<Item = (SpectaID, &'a NamedDataType)> + 'a) -> TypeMap {
    let mut type_map = TypeMap::default();

    for (sid, ndt) in iter {
        type_map.insert(sid, ndt.clone());
    }

    type_map
}

type HardcodedRuntime = tauri::Wry;

// Standalone export functions for
impl<TLang, TCommands, TEvents> Builder<TLang, TCommands, TEvents>
where
    TLang: ExportLanguage,
    TCommands: CommandsTypeState<Runtime = HardcodedRuntime>,
    TEvents: EventsTypeState,
{
    /// Exports the output of [`internal::render`] for a collection of [`FunctionDataType`] into a TypeScript file.
    pub fn export(self) -> Result<(), TLang::Error> {
        self.build_inner().map(|_| ())
    }

    pub fn export_for_plugin(mut self, plugin_name: &'static str) -> Result<(), TLang::Error> {
        self.config.plugin_name = Some(PluginName::new(plugin_name));

        self.export()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PluginName(&'static str);

pub(crate) enum ItemType {
    Event,
    Command,
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
    pub(crate) plugin_name: Option<PluginName>,
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
