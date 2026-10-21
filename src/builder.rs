use std::{any::TypeId, borrow::Cow, collections::BTreeMap, fmt, path::Path, sync::Arc};

use crate::{Casing, Commands, EventRegistry, Events, LanguageExt, event::EventRegistryMeta};
use serde::Serialize;
use specta::{
    Type, Types,
    datatype::{Function, Reference},
};
#[cfg(any(feature = "javascript", feature = "typescript"))]
use specta_typescript::semantic;
use tauri::{Manager, Runtime, ipc::Invoke};

/// The mode which the error handling is done in the bindings.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum ErrorHandlingMode {
    /// Errors will be thrown.
    ///
    /// This is the default and matches what a plain [`tauri::command`] does: a failed
    /// command rejects the returned promise. Command results are typed as `T`, and the
    /// error type is not represented in the bindings.
    #[default]
    Throw,
    /// Errors will be returned as a status-tagged result object.
    ///
    /// Successful commands return `{ status: "ok"; data: T }`, failed commands return
    /// `{ status: "error"; error: E }`.
    Result,
    /// Errors will be returned as a status-tagged result object, with the error
    /// typed as `unknown`.
    ///
    /// This is the same shape as [`ErrorHandlingMode::Result`] — successful
    /// commands return `{ status: "ok"; data: T }`, failed commands return
    /// `{ status: "error"; error: unknown }` — but the injected runtime never
    /// rethrows, so every rejection is delivered inside the result object.
    ///
    /// A rejected `invoke` may carry the command's own error value or a
    /// framework-level failure (an unknown command, a badly typed argument, an
    /// ACL denial) and the two are indistinguishable at the JavaScript
    /// boundary. [`ErrorHandlingMode::Result`] assumes the value is the
    /// command's declared error type and rethrows anything that is an `Error`
    /// instance, which escapes the result union. Use this mode when you want
    /// exhaustive handling with no escape hatch and are happy to narrow the
    /// error yourself.
    ResultUnknown,
    /// Errors will be returned as a data/error result object.
    ///
    /// Successful commands return `{ data: T, error: null }`, while failed
    /// commands return `{ data: null, error: E }`.
    DataError,
}

/// How command accessors are exposed in the generated bindings.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandExports {
    /// Commands are grouped into the exported `commands` object.
    ///
    /// This is the default: each command is a property, e.g. `commands.helloWorld()`.
    #[default]
    Namespaced,
    /// Each command is exported as its own top-level binding.
    ///
    /// A command becomes a named export rather than a property of `commands`, so a
    /// consumer can write `import { helloWorld } from "./bindings"`. This suits plugins,
    /// whose bindings are imported directly instead of through a hand-written wrapper.
    ///
    /// Accessors still follow [`Builder::function_casing`]. A casing that produces an
    /// invalid JavaScript identifier (for example [`Casing::KebabCase`]) fails the export
    /// instead of emitting an invalid binding. The `events` object and exported constants
    /// are unchanged.
    TopLevel,
}

/// How command accessor names are derived in the generated bindings.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandAccessors {
    /// Named after the command's Rust identifier, e.g. `commands.helloWorld()`.
    ///
    /// This is the default.
    #[default]
    Flat,
    /// Prefixed with the module the command was registered from.
    ///
    /// `collect_commands![some_module::greet]` becomes `commands.someModuleGreet()`, so a
    /// command can keep a short Rust name while the accessor still says which module it came
    /// from, instead of encoding the module in the function name. The module path is the one
    /// written in `collect_commands!`, and [`Builder::function_casing`] is applied to the
    /// combined name.
    ModuleIndexed,
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
/// ```rust,no_run
/// use tauri_specta::{collect_commands, collect_events, Builder};
/// use specta_typescript::Typescript;
///
///
/// let mut builder = Builder::new()
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
///     .run(tauri::test::mock_context(tauri::test::noop_assets()))
///     .expect("error while running tauri application");
/// ```
///
/// # Exporting using JSDoc
///
/// ```rust,no_run
/// use tauri_specta::{collect_commands,collect_events,Builder};
/// use specta_typescript::JSDoc;
///
///
/// let mut builder = Builder::new()
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
///     .run(tauri::test::mock_context(tauri::test::noop_assets()))
///     .expect("error while running tauri application");
/// ```
#[non_exhaustive]
pub struct Builder<R: Runtime> {
    /// The accumulated invoke handler. Every [`Builder::commands`] call chains onto this so
    /// commands registered across multiple calls are all dispatched.
    // Bounds copied from `tauri::Builder::invoke_handler`.
    invoke_handler: Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    cfg: BuilderConfiguration,
}

impl<R: Runtime> fmt::Debug for Builder<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Builder").field("cfg", &self.cfg).finish()
    }
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
/// Serializable builder state used to configure exported commands, events, and types.
pub struct BuilderConfiguration {
    /// Plugin name used when generating bindings for plugin commands.
    pub plugin_name: Option<&'static str>,
    /// Commands registered on the builder.
    pub commands: Vec<Function>,
    /// Maps each renamed command's effective Tauri name back to the Rust identifier its
    /// generated frontend accessor is derived from. Populated for commands that use
    /// `#[tauri::command(rename = "...")]`.
    pub command_accessor_names: BTreeMap<Cow<'static, str>, Cow<'static, str>>,
    /// Maps each command's effective Tauri name to the module path it was registered from.
    /// Commands registered from the crate root are absent.
    pub command_module_paths: BTreeMap<Cow<'static, str>, Cow<'static, str>>,
    /// Error handling mode used by generated bindings.
    pub error_handling: ErrorHandlingMode,
    /// Event names mapped to their type metadata.
    pub events: BTreeMap<&'static str, (TypeId, Reference)>,
    /// Collected Specta types referenced by commands, events, and manual registrations.
    pub types: Types,
    /// Constants exported alongside generated bindings.
    pub constants: BTreeMap<Cow<'static, str>, serde_json::Value>,
    /// Implementation source used for typed frontend error helpers.
    pub typed_error_impl: Cow<'static, str>,
    /// Semantic type handling configuration for supported exporters.
    #[cfg(any(feature = "javascript", feature = "typescript"))]
    pub semantic_types: Option<semantic::Configuration>,
    /// Whether BigInt-style types should be exported as TypeScript `number`.
    #[cfg(any(feature = "javascript", feature = "typescript"))]
    pub dangerously_cast_bigints_to_number: bool,
    /// Whether serde serialize/deserialize phase differences should be ignored.
    pub disable_serde_phases: bool,
    /// Casing applied to generated command and event accessor names.
    pub function_casing: Casing,
    /// How command accessor names are derived.
    pub command_accessors: CommandAccessors,
    /// How command accessors are exposed in generated bindings.
    pub command_exports: CommandExports,
}

impl BuilderConfiguration {
    /// The name a command's generated frontend accessor is derived from, before casing.
    ///
    /// This is the command's Rust identifier even when `#[tauri::command(rename = "...")]`
    /// changed the IPC name the bindings invoke. With [`CommandAccessors::ModuleIndexed`] the
    /// module the command was registered from is prefixed, joined by `_` so
    /// [`Builder::function_casing`] sees a single name.
    pub fn command_accessor_name<'a>(&'a self, command: &'a Function) -> Cow<'a, str> {
        let ident = self
            .command_accessor_names
            .get(command.name())
            .map_or_else(|| command.name(), |name| name.as_ref());

        match self.command_accessors {
            CommandAccessors::Flat => Cow::Borrowed(ident),
            CommandAccessors::ModuleIndexed => self
                .command_module_paths
                .get(command.name())
                .filter(|module_path| !module_path.is_empty())
                .map_or_else(
                    || Cow::Borrowed(ident),
                    |module_path| Cow::Owned(format!("{module_path}_{ident}")),
                ),
        }
    }
}

impl<R: Runtime> Default for Builder<R> {
    fn default() -> Self {
        Self {
            invoke_handler: Arc::new(tauri::generate_handler![]),
            cfg: Default::default(),
        }
    }
}

impl<R: Runtime> Clone for Builder<R> {
    fn clone(&self) -> Self {
        Self {
            invoke_handler: self.invoke_handler.clone(),
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
    /// Commands registered by an earlier call are kept, so this can be called more than
    /// once to accumulate commands (for example one call per module).
    ///
    /// # Panics
    ///
    /// Panics if the same command is registered twice. Generated bindings key commands by
    /// name, so a duplicate would silently keep only the last registration. This is checked
    /// here rather than at export so it also fires in release builds, which usually don't
    /// export bindings.
    ///
    /// # Example
    ///
    /// ```rust
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
    #[allow(clippy::panic)]
    pub fn commands(mut self, commands: Commands<R>) -> Self {
        let Commands(next_handler, collector) = commands;
        let collected = collector(&mut self.cfg.types);
        self.cfg.commands.extend(collected.functions);
        self.cfg
            .command_accessor_names
            .extend(collected.accessor_names);
        self.cfg.command_module_paths.extend(collected.module_paths);
        if let Some(name) = crate::commands::duplicate_command_name(&self.cfg.commands) {
            panic!(
                "Command '{name}' is registered multiple times. Remove the duplicate registration."
            );
        }

        // Chain the invoke handlers so commands from every call are dispatched. A command
        // name is registered at most once (checked above), so only one handler can match a
        // given invoke.
        let previous = self.invoke_handler;
        self.invoke_handler = Arc::new(move |invoke: Invoke<R>| {
            (previous)(Invoke {
                message: invoke.message.clone(),
                resolver: invoke.resolver.clone(),
                acl: invoke.acl.clone(),
            }) || (next_handler)(invoke)
        });

        self
    }

    /// Register events with the builder.
    ///
    /// Events registered by an earlier call are kept, so this can be called more than once
    /// to accumulate events.
    ///
    /// # Panics
    ///
    /// Panics if the same event is registered twice. Generated bindings key events by name,
    /// so a duplicate would silently keep only the last registration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use serde::{Serialize, Deserialize};
    /// use specta::Type;
    /// use tauri_specta::{Builder, collect_events, Event};
    ///
    /// #[derive(Serialize, Deserialize, Debug, Clone, Type, Event)]
    /// pub struct DemoEvent(String);
    ///
    /// let mut builder = Builder::<tauri::Wry>::new().events(collect_events![DemoEvent]);
    /// ```
    #[allow(clippy::panic)]
    pub fn events(mut self, events: Events) -> Self {
        for (name, build) in events.0 {
            if self.cfg.events.contains_key(name) {
                panic!(
                    "Event '{name}' is registered multiple times. Remove the duplicate registration."
                );
            }

            self.cfg.events.insert(name, build(&mut self.cfg.types));
        }

        self
    }

    /// Export a new type with the frontend.
    ///
    /// This is useful if you want to export types that do not appear in any events or commands.
    ///
    /// # Example
    ///
    /// ```rust
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

    /// Export a group of types to the frontend.
    ///
    /// This is useful if you want to export types that do not appear in any events or commands.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tauri_specta::Builder;
    /// use specta::{Type, Types};
    ///
    /// let mut builder = Builder::<tauri::Wry>::new().types(&Types::default());
    /// ```
    pub fn types(mut self, types: &Types) -> Self {
        self.cfg.types.extend(types);
        self
    }

    /// Export a constant value to the frontend.
    ///
    /// This is useful to share application-wide constants or expose data which is generated by Rust.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tauri_specta::Builder;
    ///
    /// let mut builder = Builder::<tauri::Wry>::new().constant("CONSTANT_NAME","ANY_CONSTANT_VALUE");
    /// ```
    #[track_caller]
    pub fn constant<T: Serialize>(mut self, k: impl Into<Cow<'static, str>>, v: T) -> Self {
        self.cfg.constants.insert(
            k.into(),
            serde_json::to_value(v).expect("Tauri Specta failed to serialize constant"),
        );
        self
    }

    /// Set the error handling mode for the generated bindings.
    ///
    /// [`ErrorHandlingMode::Result`] and [`ErrorHandlingMode::DataError`] inject a runtime
    /// that rethrows when the rejected value is an `Error` instance — a genuine
    /// JavaScript-level failure rather than a value the command returned — instead of
    /// returning it in the result object. [`ErrorHandlingMode::ResultUnknown`] (or a custom
    /// [`Builder::typed_error_impl`]) is how you opt out of that.
    ///
    /// Note that framework-level Tauri failures (an unknown command, a badly typed
    /// argument, an ACL denial) reject with a free-form string, so a command's declared
    /// error type `E` only describes errors the command itself returned.
    pub fn error_handling(mut self, error_handling: ErrorHandlingMode) -> Self {
        self.cfg.error_handling = error_handling;
        self
    }

    /// Replace the internal implementation of the `typedError` function.
    /// This would allow integrating with Effect or any other result library.
    /// The implementation must return the shape selected by [`ErrorHandlingMode`].
    ///
    /// The built-in runtime rethrows when the rejection is an `Error` instance (a genuine
    /// JavaScript-level failure) rather than returning it in the result object. A custom
    /// implementation is how you opt out of that, alongside
    /// [`ErrorHandlingMode::ResultUnknown`].
    ///
    /// ```rust
    /// use tauri_specta::Builder;
    ///
    /// const TYPED_ERROR_IMPL: &str = r#"async function typedError<T, E>(result: Promise<T>): Promise<{ status: "ok"; data: T } | { status: "error"; error: E }> {
    ///     try {
    ///         return { status: "ok", data: await result };
    ///     } catch (e) {
    ///         if (e instanceof Error) throw e;
    ///         return { status: "error", error: e as any };
    ///     }
    /// }"#;
    ///
    /// Builder::<tauri::Wry>::default()
    ///  .typed_error_impl(TYPED_ERROR_IMPL);
    /// ```
    pub fn typed_error_impl(mut self, runtime: impl Into<Cow<'static, str>>) -> Self {
        self.cfg.typed_error_impl = runtime.into();
        self
    }

    /// Enable semantic frontend type handling for exported bindings.
    ///
    /// This opts into runtime transforms and type remapping for transport-specific
    /// shapes such as bigints or custom semantic types.
    ///
    /// See the [`specta_typescript::semantic`](https://docs.rs/specta-typescript/latest/specta_typescript/semantic/index.html)
    /// docs for configuration details.
    #[cfg(any(feature = "javascript", feature = "typescript"))]
    pub fn semantic_types(mut self, semantic_types: semantic::Configuration) -> Self {
        self.cfg.semantic_types = Some(semantic_types);
        self
    }

    /// Dangerously export BigInt-style types as TypeScript `number`.
    ///
    /// This is dangerous as large numbers may be truncated or lose precision. Refer to the [upstream guidance](https://docs.rs/specta-typescript/latest/specta_typescript/struct.Error.html#bigint-forbidden) for more information.
    #[cfg(any(feature = "javascript", feature = "typescript"))]
    pub fn dangerously_cast_bigints_to_number(mut self) -> Self {
        self.cfg.dangerously_cast_bigints_to_number = true;
        self
    }

    /// Disable phase-aware serde export handling and use unified serde mode.
    ///
    /// This causes export to use `specta_serde::apply` instead of
    /// `specta_serde::apply_phases`.
    pub fn disable_serde_phases(mut self) -> Self {
        self.cfg.disable_serde_phases = true;
        self
    }

    /// Set the casing convention used for generated command and event accessor names.
    ///
    /// By default Tauri Specta renames Rust `snake_case` names to [`Casing::CamelCase`]
    /// (e.g. `hello_world` becomes `commands.helloWorld`). Use this to keep the original
    /// Rust naming or pick another convention.
    ///
    /// This does not affect the underlying Tauri command/event string used to invoke the
    /// command, only the JavaScript accessor name in the generated bindings.
    ///
    /// Command argument names are separate because they are part of Tauri's
    /// runtime IPC contract. To rename arguments, configure the command itself
    /// and keep `#[tauri::command(rename_all = "...")]` aligned with
    /// `#[specta(rename_all = "...")]`. This method never renames arguments.
    ///
    /// ```rust
    /// use tauri_specta::{Builder, Casing};
    ///
    /// let mut builder = Builder::<tauri::Wry>::new().function_casing(Casing::SnakeCase);
    /// ```
    pub fn function_casing(mut self, casing: Casing) -> Self {
        self.cfg.function_casing = casing;
        self
    }

    /// Set how command accessor names are derived in the generated bindings.
    ///
    /// The default is [`CommandAccessors::Flat`], which names each command after its Rust
    /// identifier. [`CommandAccessors::ModuleIndexed`] prefixes the module the command was
    /// registered from, so `collect_commands![some_module::greet]` becomes
    /// `commands.someModuleGreet()`.
    ///
    /// ```rust
    /// use tauri_specta::{Builder, CommandAccessors};
    ///
    /// let mut builder = Builder::<tauri::Wry>::new()
    ///     .command_accessors(CommandAccessors::ModuleIndexed);
    /// ```
    pub fn command_accessors(mut self, command_accessors: CommandAccessors) -> Self {
        self.cfg.command_accessors = command_accessors;
        self
    }

    /// Set how command accessors are exposed in the generated bindings.
    ///
    /// The default is [`CommandExports::Namespaced`], which groups commands into the
    /// exported `commands` object. [`CommandExports::TopLevel`] exports each command as
    /// its own named binding instead, which is useful for plugins.
    ///
    /// ```rust
    /// use tauri_specta::{Builder, CommandExports};
    ///
    /// let mut builder = Builder::<tauri::Wry>::new()
    ///     .command_exports(CommandExports::TopLevel);
    /// ```
    pub fn command_exports(mut self, command_exports: CommandExports) -> Self {
        self.cfg.command_exports = command_exports;
        self
    }

    /// The Tauri invoke handler to trigger commands registered with the builder.
    pub fn invoke_handler(&self) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
        let handler = self.invoke_handler.clone();
        move |invoke| handler(invoke)
    }

    /// Mount all of the events in the builder onto a Tauri app.
    ///
    /// This should be called within [`tauri::Builder::setup`](tauri::Builder::setup) like the example below.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use tauri_specta::{Builder, collect_events};
    ///
    /// let mut builder = Builder::new().events(collect_events![]);
    ///
    /// tauri::Builder::default()
    ///     .setup(move |app| {
    ///         builder.mount_events(app);
    ///
    ///         Ok(())
    ///     })
    ///     .run(tauri::test::mock_context(tauri::test::noop_assets()))
    ///     .expect("error while running tauri application");
    /// ```
    pub fn mount_events(&self, handle: &impl Manager<R>) {
        let registry = EventRegistry::get_or_manage(handle);
        let mut map = registry.0.write().expect("Failed to lock EventRegistry");

        for (tid, _) in self.cfg.events.values() {
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
    /// ```rust
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
