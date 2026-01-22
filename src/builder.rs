use std::{any::TypeId, borrow::Cow, collections::BTreeMap, path::Path};

use crate::{Commands, EventRegistry, Events, LanguageExt, event::EventRegistryMeta};
use serde::Serialize;
use specta::{
    Type, TypeCollection,
    datatype::{DataType, Function},
};
use tauri::{Manager, Runtime, ipc::Invoke};

/// The mode which the error handling is done in the bindings.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum ErrorHandlingMode {
    /// Errors will be thrown
    Throw,
    /// Errors will be returned as a Result enum
    #[default]
    Result,
}

/// Builder for configuring Tauri Specta in your application.
///
/// # Example
///
/// You can copy the following code into your `main.rs` file to get started with Tauri Specta.
///
/// This will automatically export a [Typescript](https://www.typescriptlang.org) file containing bindings for all of your commands and events.
///
/// You can extend this example by calling other methods on the builder to configure your application further.
///
/// ```rust,ignore
/// use tauri_specta::{collect_commands, collect_events, Builder};
/// use specta_typescript::Typescript;
///
///
/// let mut builder = <Builder>::new()
///     .commands(collect_commands![])
///     .events(collect_events![]);
///
/// #[cfg(debug_assertions)]
/// builder
///     .export(Typescript::default(), "../src/bindings.ts")
///     .expect("Failed to export typescript bindings");
///
/// tauri::Builder::default()
///     .invoke_handler(builder.invoke_handler()) // < Required for commands to work
///     .setup(move |app| {
///         builder.mount_events(app); // < Required for events to work
///
///         Ok(())
///     })
///     // on an actual app, remove the string argument
///     .run(tauri::generate_context!("tests/tauri.conf.json"))
///     .expect("error while running tauri application");
/// ```
///
/// # Exporting using JSDoc
///
/// ```rust,ignore
/// use tauri_specta::{collect_commands,collect_events,Builder};
/// use specta_jsdoc::JSDoc;
///
///
/// let mut builder = <Builder>::new()
///     .commands(collect_commands![])
///     .events(collect_events![]);
///
/// // exporting to JsDoc
/// #[cfg(debug_assertions)]
/// builder
///     .export(JSDoc::default(), "../src/bindings.js")
///     .expect("Failed to export jsdoc bindings");
///
/// tauri::Builder::default()
///     .invoke_handler(builder.invoke_handler()) // < Required for commands to work
///     .setup(move |app| {
///         builder.mount_events(app); // < Required for events to work
///
///         Ok(())
///     })
///     // on an actual app, remove the string argument
///     .run(tauri::generate_context!("tests/tauri.conf.json"))
///     .expect("error while running tauri application");
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub struct Builder<R: Runtime = tauri::Wry> {
    commands: Commands<R>,
    cfg: BuilderConfiguration,
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct BuilderConfiguration {
    pub plugin_name: Option<&'static str>,
    pub commands: Vec<Function>,
    pub error_handling: ErrorHandlingMode,
    pub events: BTreeMap<&'static str, (TypeId, DataType)>,
    pub types: TypeCollection,
    pub constants: BTreeMap<Cow<'static, str>, serde_json::Value>,
    pub as_result_impl: Cow<'static, str>,
}

impl<R: Runtime> Default for Builder<R> {
    fn default() -> Self {
        Self {
            commands: Default::default(),
            cfg: Default::default(),
        }
    }
}

impl<R: Runtime> Clone for Builder<R> {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            cfg: self.cfg.clone(),
        }
    }
}

impl<R: Runtime> Builder<R> {
    /// Construct a new Tauri Specta builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the name of the current plugin name.
    ///
    /// This is used to ensure the generated bindings correctly reference the plugin.
    pub fn plugin_name(mut self, plugin_name: &'static str) -> Self {
        self.cfg.plugin_name = Some(plugin_name);
        self
    }

    /// Register commands with the builder.
    ///
    /// **WARNING:** This method will overwrite any previously registered commands.
    ///
    /// # Example
    ///
    /// ```rust,ignore-windows
    /// use tauri_specta::{Builder, collect_commands};
    ///
    /// #[tauri::command]
    /// #[specta::specta]
    /// fn hello_world(my_name: String) -> String {
    ///     format!("Hello, {my_name}! You've been greeted from Rust!")
    /// }
    ///
    /// let mut builder = Builder::<tauri::Wry>::new().commands(collect_commands![hello_world]);
    /// ```
    pub fn commands(mut self, commands: Commands<R>) -> Self {
        self.cfg.commands = (commands.1)(&mut self.cfg.types);

        Self {
            commands,
            cfg: self.cfg,
        }
    }

    /// Register events with the builder.
    ///
    /// **WARNING:** This method will overwrite any previously registered events.
    ///
    /// # Example
    ///
    /// ```rust,ignore-windows
    /// use serde::{Serialize, Deserialize};
    /// use specta::Type;
    /// use tauri_specta::{Builder, collect_events, Event};
    ///
    /// #[derive(Serialize, Deserialize, Debug, Clone, Type, Event)]
    /// pub struct DemoEvent(String);
    ///
    /// let mut builder = Builder::<tauri::Wry>::new().events(collect_events![DemoEvent]);
    /// ```
    pub fn events(mut self, events: Events) -> Self {
        self.cfg.events = events
            .0
            .iter()
            .map(|(k, build)| (*k, build(&mut self.cfg.types)))
            .collect();
        self
    }

    /// Export a new type with the frontend.
    ///
    /// This is useful if you want to export types that do not appear in any events or commands.
    ///
    /// # Example
    ///
    /// ```rust,ignore-windows
    /// use tauri_specta::Builder;
    /// use serde::{Serialize, Deserialize};
    /// use specta::Type;
    ///
    /// #[derive(Serialize, Deserialize, Type)]
    /// pub struct MyStruct {
    ///     a: String
    /// }
    ///
    /// let mut builder = Builder::<tauri::Wry>::new().typ::<MyStruct>();
    /// ```
    pub fn typ<T: Type>(mut self) -> Self {
        self.cfg.types.register_mut::<T>();
        self
    }

    /// Export a constant value to the frontend.
    ///
    /// This is useful to share application-wide constants or expose data which is generated by Rust.
    ///
    /// # Example
    ///
    /// ```rust,ignore-windows
    /// use tauri_specta::Builder;
    ///
    /// let mut builder = Builder::<tauri::Wry>::new().constant("CONSTANT_NAME","ANY_CONSTANT_VALUE");
    /// ```
    #[track_caller]
    pub fn constant<T: Serialize + Type>(mut self, k: impl Into<Cow<'static, str>>, v: T) -> Self {
        self.cfg.constants.insert(
            k.into(),
            serde_json::to_value(v).expect("Tauri Specta failed to serialize constant"),
        );
        self
    }

    /// Set the error handling mode for the generated bindings.
    pub fn error_handling(mut self, error_handling: ErrorHandlingMode) -> Self {
        self.cfg.error_handling = error_handling;
        self
    }

    // TODO: Maybe method to merge in a `TypeCollection`

    // TODO: Should we put a `.build` command here to ensure it's immutable from now on?

    /// The Tauri invoke handler to trigger commands registered with the builder.
    pub fn invoke_handler(&self) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
        let commands = self.commands.0.clone();
        move |invoke| commands(invoke)
    }

    /// Mount all of the events in the builder onto a Tauri app.
    ///
    /// This should be called within [`tauri::Builder::setup`](tauri::Builder::setup) like the example below.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use tauri_specta::{Builder, collect_events};
    ///
    /// let mut builder = Builder::<tauri::Wry>::new().events(collect_events![]);
    ///
    /// tauri::Builder::default()
    ///     .setup(move |app| {
    ///         builder.mount_events(app);
    ///
    ///         Ok(())
    ///     })
    ///     // on an actual app, remove the string argument
    ///     .run(tauri::generate_context!("tests/tauri.conf.json"))
    ///     .expect("error while running tauri application");
    /// ```
    pub fn mount_events(&self, handle: &impl Manager<R>) {
        let registry = EventRegistry::get_or_manage(handle);
        let mut map = registry.0.write().expect("Failed to lock EventRegistry");

        for (_, (tid, _)) in &self.cfg.events {
            map.insert(
                *tid,
                EventRegistryMeta {
                    plugin_name: self.cfg.plugin_name,
                },
            );
        }
    }

    /// Export the bindings to the filesystem using the provided exporter.
    ///
    /// # Example
    /// ```rust,ignore-windows
    /// use tauri_specta::{Builder, collect_commands, collect_events};
    /// use specta_typescript::Typescript;
    ///
    /// let mut builder = Builder::<tauri::Wry>::new()
    ///     .commands(collect_commands![])
    ///     .events(collect_events![]);
    ///
    /// #[cfg(debug_assertions)] // only export on debug builds.
    /// builder
    ///     .export(Typescript::default(), "../src/bindings.ts")
    ///     .expect("Failed to export typescript bindings");
    /// ```
    pub fn export<L: LanguageExt>(
        &self,
        language: L,
        path: impl AsRef<Path>,
    ) -> Result<(), L::Error> {
        language.export(&self.cfg, path.as_ref())
    }
}
