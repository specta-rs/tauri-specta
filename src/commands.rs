use std::{borrow::Cow, collections::BTreeSet, fmt, sync::Arc};

#[cfg(feature = "introspection")]
use std::future::Future;

#[cfg(not(feature = "introspection"))]
use heck::ToLowerCamelCase;
#[cfg(feature = "introspection")]
use specta::Type;
#[cfg(not(feature = "introspection"))]
use specta::datatype::Function;
use specta::{
    Types,
    datatype::{DataType, Deprecated},
};
use tauri::{Runtime, ipc::Invoke};

/// A command as seen by the binding exporters.
///
/// Produced by `collect_commands!` and exposed through
/// [`BuilderConfiguration::commands`](crate::BuilderConfiguration::commands).
#[derive(Debug, Clone)]
pub struct Command {
    /// The command name Tauri's invoke handler accepts, after
    /// `#[tauri::command(rename = "...")]`. The plugin prefix is added at export time.
    pub name: Cow<'static, str>,
    /// The Rust identifier the frontend accessor is derived from.
    pub accessor: Cow<'static, str>,
    /// The arguments the frontend sends, in declaration order.
    pub args: Vec<CommandArg>,
    /// The command result, or `None` when it resolves to `void`.
    pub result: Option<DataType>,
    /// Rust-doc comments on the command.
    pub docs: Cow<'static, str>,
    /// Rust deprecation metadata on the command.
    pub deprecated: Option<Deprecated>,
}

/// A frontend-visible command argument.
#[derive(Debug, Clone)]
pub struct CommandArg {
    /// The key Tauri reads this argument from, after `rename_all`.
    pub name: Cow<'static, str>,
    /// The frontend type of the argument.
    pub ty: DataType,
}

/// The command metadata produced by a [`CommandTypeCollector`].
#[doc(hidden)]
#[derive(Default)]
pub struct CollectedCommands {
    /// The registered commands, in registration order.
    pub commands: Vec<Command>,
}

/// Type-erased collector used by [`Commands`] to register command metadata.
#[doc(hidden)]
pub type CommandTypeCollector = dyn Fn(&mut Types) -> CollectedCommands + Send + Sync + 'static;

/// A wrapper around the output of the `collect_commands` macro.
///
/// This acts to seal the implementation details of the macro.
pub struct Commands<R: Runtime>(
    // TODO: Explain these being public
    // Bounds copied from `tauri::Builder::invoke_handler`
    pub Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    pub Arc<CommandTypeCollector>,
);

impl<R: Runtime> fmt::Debug for Commands<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Commands").finish()
    }
}

impl<R: Runtime> Default for Commands<R> {
    fn default() -> Self {
        Self(
            Arc::new(tauri::generate_handler![]),
            Arc::new(|_: &mut Types| CollectedCommands::default()),
        )
    }
}

impl<R: Runtime> Clone for Commands<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

/// The first command name registered more than once, if any.
///
/// Checked when commands are registered rather than when bindings are exported, so the
/// mistake is caught in release builds too — `export` is usually `#[cfg(debug_assertions)]`.
/// Collisions between *differently* named commands are a separate, export-time check,
/// because the accessor they share depends on the final `function_casing`.
pub(crate) fn duplicate_command_name(commands: &[Command]) -> Option<&str> {
    let mut seen = BTreeSet::new();
    for command in commands {
        if !seen.insert(command.name.as_ref()) {
            return Some(command.name.as_ref());
        }
    }
    None
}

/// Convert Specta's function metadata into commands.
///
/// Used by the pre-introspection path (Tauri v3), where Tauri has no command metadata:
/// Specta names each function after its Rust identifier, and `renames` pairs that
/// identifier with the name Tauri's hidden `__tauri_command_name_*!()` macro reports.
///
/// # Panics
///
/// Panics if a renamed command doesn't correspond to exactly one collected function,
/// rather than silently leaving the binding with a name the handler rejects.
#[cfg(not(feature = "introspection"))]
#[allow(clippy::panic)]
pub(crate) fn from_functions(
    functions: Vec<Function>,
    renames: &[(&'static str, &'static str)],
) -> CollectedCommands {
    let mut commands: Vec<Command> = functions
        .into_iter()
        .map(|function| Command {
            accessor: function.name.clone(),
            name: function.name,
            args: function
                .args
                .into_iter()
                .map(|(name, ty)| CommandArg {
                    // Without Tauri's metadata the best available key is the Rust
                    // parameter name under Tauri's default `camelCase` argument casing.
                    name: Cow::Owned(name.to_lower_camel_case()),
                    ty,
                })
                .collect(),
            result: function.result.and_then(top_level_unit_as_void),
            docs: normalize_specta_docs(function.docs),
            deprecated: function.deprecated,
        })
        .collect();

    // Snapshot the accessor names so a rename can't match a command an earlier rename
    // already touched.
    let original: Vec<Cow<'static, str>> = commands
        .iter()
        .map(|command| command.accessor.clone())
        .collect();

    for (rust_name, tauri_name) in renames {
        // Without an explicit `rename`, Tauri's name is the Rust identifier and the
        // accessor already matches.
        if rust_name == tauri_name {
            continue;
        }

        let mut matched = None;
        for (index, name) in original.iter().enumerate() {
            if name == rust_name {
                if matched.is_some() {
                    panic!(
                        "Command '{rust_name}' is registered more than once, so its `#[tauri::command(rename = \"{tauri_name}\")]` can't be applied unambiguously."
                    );
                }
                matched = Some(index);
            }
        }

        match matched {
            Some(index) => commands[index].name = Cow::Borrowed(tauri_name),
            None => panic!(
                "Command '{rust_name}' has `#[tauri::command(rename = \"{tauri_name}\")]`, but no collected command has that name. Make sure the command is also annotated with `#[specta::specta]`."
            ),
        }
    }

    CollectedCommands { commands }
}

/// Strip the leading space each `///` line contributes.
///
/// Specta keeps the space; Tauri's command metadata strips one per line. Normalising
/// here keeps bindings identical between the two collection paths.
#[cfg(not(feature = "introspection"))]
fn normalize_specta_docs(docs: Cow<'static, str>) -> Cow<'static, str> {
    if !docs.contains(' ') && !docs.contains('\n') {
        return docs;
    }

    Cow::Owned(
        docs.split('\n')
            .map(|line| line.strip_prefix(' ').unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// A Tauri command whose frontend types are inferred from its Rust signature.
///
/// Implemented for function references that Tauri's `#[tauri::command]` macro accepts,
/// so `collect_commands!` no longer needs `#[specta::specta]` on each command. Every
/// argument the frontend sends, and the result, must implement `specta::Type`, and a
/// command may take at most ten arguments (the same limit as Specta's own function
/// collection). The marker parameter keeps the argument-count and result-kind
/// implementations from overlapping.
#[cfg(feature = "introspection")]
#[doc(hidden)]
pub trait CommandSignature<TMarker> {
    /// The command's argument tuple.
    type Args;
    /// Marker types selecting an argument implementation per position.
    type ArgMarkers;
    /// The command's declared result type.
    type Result;
    /// Marker selecting a synchronous or asynchronous result implementation.
    type ResultMarker;

    /// Convert the command into binding metadata using Tauri's command metadata.
    fn into_command(
        self,
        metadata: tauri::ipc::CommandMetadata,
        accessor: Cow<'static, str>,
        types: &mut Types,
    ) -> Command
    where
        Self::Args: CommandArguments<Self::ArgMarkers>,
        Self::Result: CommandResult<Self::ResultMarker>;
}

/// All of a command's arguments, as a tuple.
#[cfg(feature = "introspection")]
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "one or more Tauri command arguments cannot be exported by specta",
    note = "derive or implement `specta::Type` for every argument the frontend sends",
    note = "`State`, `AppHandle`, `Window`, `Webview` and `WebviewWindow` are injected by Tauri and are not sent from the frontend"
)]
pub trait CommandArguments<TMarker> {
    /// One entry per argument, in declaration order. `None` marks an injected argument.
    fn to_datatypes(types: &mut Types) -> Vec<Option<DataType>>;
}

/// A single command argument.
#[cfg(feature = "introspection")]
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be exported as a Tauri command argument",
    label = "this argument is missing `specta::Type`",
    note = "derive or implement `specta::Type` for `{Self}`",
    note = "if Tauri injects this argument instead of reading it from the invoke payload, please report it: tauri-specta does not know about it"
)]
pub trait CommandArgument<TMarker> {
    /// `None` when Tauri injects the argument rather than sending it from the frontend.
    fn to_datatype(types: &mut Types) -> Option<DataType>;
}

/// Marker for arguments the frontend sends.
#[cfg(feature = "introspection")]
#[doc(hidden)]
pub enum SpectaArgumentMarker {}

#[cfg(feature = "introspection")]
impl<T: Type> CommandArgument<SpectaArgumentMarker> for T {
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        Some(T::definition(types))
    }
}

/// Marker for arguments Tauri injects.
#[cfg(feature = "introspection")]
#[doc(hidden)]
pub enum InjectedArgumentMarker {}

#[cfg(feature = "introspection")]
impl<T: Send + Sync + 'static> CommandArgument<InjectedArgumentMarker> for tauri::State<'_, T> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

#[cfg(feature = "introspection")]
impl<R: Runtime> CommandArgument<InjectedArgumentMarker> for tauri::AppHandle<R> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

#[cfg(feature = "introspection")]
impl<R: Runtime> CommandArgument<InjectedArgumentMarker> for tauri::Window<R> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

#[cfg(feature = "introspection")]
impl<R: Runtime> CommandArgument<InjectedArgumentMarker> for tauri::Webview<R> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

#[cfg(feature = "introspection")]
impl<R: Runtime> CommandArgument<InjectedArgumentMarker> for tauri::WebviewWindow<R> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

/// A command's declared result type.
#[cfg(feature = "introspection")]
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be exported as a Tauri command result",
    label = "this result needs `specta::Type`",
    note = "derive or implement `specta::Type` for `{Self}`",
    note = "for an `async fn`, the type to annotate is the future's output: `impl Future<Output = T>` in this message means `T` needs `specta::Type`"
)]
pub trait CommandResult<TMarker> {
    /// `None` for a result that is not sent to the frontend (a top-level unit).
    fn to_datatype(types: &mut Types) -> Option<DataType>;
}

/// Marker for a synchronous result.
#[cfg(feature = "introspection")]
#[doc(hidden)]
pub enum SpectaResultMarker {}

#[cfg(feature = "introspection")]
impl<T: Type> CommandResult<SpectaResultMarker> for T {
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        top_level_unit_as_void(T::definition(types))
    }
}

/// Marker for an `async fn` result, whose value is the future's output.
#[cfg(feature = "introspection")]
#[doc(hidden)]
pub enum FutureResultMarker {}

#[cfg(feature = "introspection")]
impl<F> CommandResult<FutureResultMarker> for F
where
    F: Future,
    F::Output: AsyncCommandResult,
{
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        F::Output::to_datatype(types)
    }
}

/// The awaited output of an `async fn` command.
///
/// This exists separately from [`CommandResult`] so the diagnostic names the awaited
/// type instead of the future, which is what the author actually has to fix.
#[cfg(feature = "introspection")]
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` is the awaited return type of an async command but cannot be exported by specta",
    label = "this async command's return type needs `specta::Type`",
    note = "derive or implement `specta::Type` for `{Self}`",
    note = "an `async fn` command must resolve to a type specta understands, like a struct, `String`, `Result<T, E>`, or `()`"
)]
pub trait AsyncCommandResult {
    /// `None` for a top-level unit, which Tauri sends as `void`.
    fn to_datatype(types: &mut Types) -> Option<DataType>;
}

#[cfg(feature = "introspection")]
impl<T: Type> AsyncCommandResult for T {
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        top_level_unit_as_void(T::definition(types))
    }
}

fn top_level_unit_as_void(dt: DataType) -> Option<DataType> {
    match &dt {
        DataType::Tuple(tuple) if tuple.elements.is_empty() => None,
        _ => Some(dt),
    }
}

#[cfg(feature = "introspection")]
fn command_from_parts(
    metadata: tauri::ipc::CommandMetadata,
    accessor: Cow<'static, str>,
    arg_types: Vec<Option<DataType>>,
    result: Option<DataType>,
) -> Command {
    // Zip before filtering: `metadata.arguments` lists every argument, including the
    // ones Tauri injects, so dropping injected types first would shift the names.
    let args = metadata
        .arguments
        .iter()
        .zip(arg_types)
        .filter_map(|(name, ty)| {
            ty.filter(|_| !name.is_empty()).map(|ty| CommandArg {
                name: Cow::Borrowed(*name),
                ty,
            })
        })
        .collect();

    Command {
        name: Cow::Borrowed(metadata.name),
        accessor,
        args,
        result,
        docs: Cow::Borrowed(metadata.docs),
        deprecated: metadata.deprecated.map(|deprecated| {
            let mut out = Deprecated::new();
            out.note = deprecated.note.map(Cow::Borrowed);
            out.since = deprecated.since.map(Cow::Borrowed);
            out
        }),
    }
}

#[cfg(feature = "introspection")]
macro_rules! impl_command_signature {
    () => {
        impl CommandArguments<()> for () {
            fn to_datatypes(_: &mut Types) -> Vec<Option<DataType>> {
                Vec::new()
            }
        }

        impl<F, TResult, TResultMarker> CommandSignature<(TResult, TResultMarker)> for F
        where
            for<'a> &'a F: Fn() -> TResult,
        {
            type Args = ();
            type ArgMarkers = ();
            type Result = TResult;
            type ResultMarker = TResultMarker;

            fn into_command(
                self,
                metadata: tauri::ipc::CommandMetadata,
                accessor: Cow<'static, str>,
                types: &mut Types,
            ) -> Command
            where
                Self::Args: CommandArguments<Self::ArgMarkers>,
                Self::Result: CommandResult<Self::ResultMarker>,
            {
                let _ = self;
                command_from_parts(
                    metadata,
                    accessor,
                    Vec::new(),
                    Self::Result::to_datatype(types),
                )
            }
        }
    };
    ($($ty:ident : $marker:ident),+) => {
        impl<$($ty, $marker),+> CommandArguments<($($marker,)+)> for ($($ty,)+)
        where
            $($ty: CommandArgument<$marker>,)+
        {
            fn to_datatypes(types: &mut Types) -> Vec<Option<DataType>> {
                vec![$($ty::to_datatype(types)),+]
            }
        }

        impl<F, TResult, TResultMarker, $($ty, $marker),+>
            CommandSignature<($($ty, $marker,)+ TResult, TResultMarker)> for F
        where
            for<'a> &'a F: Fn($($ty),+) -> TResult,
        {
            type Args = ($($ty,)+);
            type ArgMarkers = ($($marker,)+);
            type Result = TResult;
            type ResultMarker = TResultMarker;

            fn into_command(
                self,
                metadata: tauri::ipc::CommandMetadata,
                accessor: Cow<'static, str>,
                types: &mut Types,
            ) -> Command
            where
                Self::Args: CommandArguments<Self::ArgMarkers>,
                Self::Result: CommandResult<Self::ResultMarker>,
            {
                let _ = self;
                command_from_parts(
                    metadata,
                    accessor,
                    Self::Args::to_datatypes(types),
                    Self::Result::to_datatype(types),
                )
            }
        }
    };
}

#[cfg(feature = "introspection")]
impl_command_signature!();
#[cfg(feature = "introspection")]
impl_command_signature!(T1: M1);
#[cfg(feature = "introspection")]
impl_command_signature!(T1: M1, T2: M2);
#[cfg(feature = "introspection")]
impl_command_signature!(T1: M1, T2: M2, T3: M3);
#[cfg(feature = "introspection")]
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4);
#[cfg(feature = "introspection")]
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5);
#[cfg(feature = "introspection")]
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5, T6: M6);
#[cfg(feature = "introspection")]
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5, T6: M6, T7: M7);
#[cfg(feature = "introspection")]
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5, T6: M6, T7: M7, T8: M8);
#[cfg(feature = "introspection")]
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5, T6: M6, T7: M7, T8: M8, T9: M9);
#[cfg(feature = "introspection")]
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5, T6: M6, T7: M7, T8: M8, T9: M9, T10: M10);
